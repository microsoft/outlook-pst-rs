//! ## [Named Property Lookup Map](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/e17e195d-0454-4b9b-b398-c9127a26a678)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::{
    collections::BTreeMap,
    fmt::Display,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    sync::Arc,
};

use super::{read_write::*, store::*, *};
use crate::{
    crc::compute_crc,
    ltp::{
        heap::{AnsiHeapNode, UnicodeHeapNode},
        prop_context::{AnsiPropertyContext, GuidValue, PropertyValue, UnicodePropertyContext},
        prop_type::PropertyType,
        read_write::PropertyValueReadWrite,
        tree::{AnsiHeapTree, UnicodeHeapTree},
    },
    ndb::{
        block::{AnsiDataTree, UnicodeDataTree},
        header::Header,
        node_id::NID_NAME_TO_ID_MAP,
        page::{
            AnsiBlockBTree, AnsiNodeBTree, NodeBTreeEntry, UnicodeBlockBTree, UnicodeNodeBTree,
        },
        root::Root,
    },
    PstFile,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NamedPropertyId {
    Number(u32),
    StringOffset(u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NamedPropertyGuid {
    None,
    Mapi,
    PublicStrings,
    GuidIndex(u16),
}

impl From<NamedPropertyGuid> for u16 {
    fn from(guid: NamedPropertyGuid) -> Self {
        match guid {
            NamedPropertyGuid::None => 0x0000,
            NamedPropertyGuid::Mapi => 0x0001,
            NamedPropertyGuid::PublicStrings => 0x0002,
            NamedPropertyGuid::GuidIndex(index) => index + 3,
        }
    }
}

impl TryFrom<u16> for NamedPropertyGuid {
    type Error = MessagingError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value & 0x8000 != 0 {
            return Err(MessagingError::NamedPropertyMapGuidIndexOutOfBounds(value));
        }

        Ok(match value {
            0x0000 => Self::None,
            0x0001 => Self::Mapi,
            0x0002 => Self::PublicStrings,
            index => Self::GuidIndex(index - 3),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NamedPropertyIndex(u16);

impl NamedPropertyIndex {
    pub fn prop_id(&self) -> u16 {
        self.0
    }
}

impl From<NamedPropertyIndex> for u16 {
    fn from(value: NamedPropertyIndex) -> Self {
        value.0 - 0x8000
    }
}

impl TryFrom<u16> for NamedPropertyIndex {
    type Error = MessagingError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value >= 0x8000 {
            return Err(MessagingError::NamedPropertyMapPropertyIndexOutOfBounds(
                value,
            ));
        }
        Ok(Self(value + 0x8000))
    }
}

pub const PS_MAPI: GuidValue = GuidValue::new(
    0x00020328,
    0x0000,
    0x0000,
    [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
);

pub const PS_PUBLIC_STRINGS: GuidValue = GuidValue::new(
    0x00020329,
    0x0000,
    0x0000,
    [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
);

#[derive(Clone, Default, Debug)]
pub struct StringEntry {
    size: u32,
    buffer: Vec<u8>,
}

impl StringEntry {
    pub fn new(size: u32, buffer: Vec<u8>) -> MessagingResult<Self> {
        if size % 2 != 0 || size as usize != buffer.len() {
            Err(MessagingError::NamedPropertyMapStringEntryOutOfBounds)
        } else {
            Ok(Self { size, buffer })
        }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

impl NamedPropReadWrite for StringEntry {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let size = f.read_u32::<LittleEndian>()?;

        if size % 2 != 0 {
            return Err(
                MessagingError::InvalidNamedPropertyMapStreamString(PropertyType::Binary).into(),
            );
        }

        let mut buffer = vec![0; size as usize];
        f.read_exact(&mut buffer)?;

        Ok(Self::new(size, buffer)?)
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.size)?;
        f.write_all(&self.buffer)
    }
}

impl Display for StringEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut cursor = Cursor::new(self.buffer.as_slice());
        let count = self.size / 2;
        let mut buffer = Vec::with_capacity(count as usize);
        for _ in 0..count {
            buffer.push(cursor.read_u16::<LittleEndian>().unwrap_or_default());
        }
        let value = String::from_utf16_lossy(&buffer);
        write!(f, "{value}")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameIdEntry {
    id: NamedPropertyId,
    guid: NamedPropertyGuid,
    prop_index: NamedPropertyIndex,
}

impl NameIdEntry {
    pub fn new(
        id: NamedPropertyId,
        guid: NamedPropertyGuid,
        prop_index: NamedPropertyIndex,
    ) -> Self {
        Self {
            id,
            guid,
            prop_index,
        }
    }

    pub fn id(&self) -> NamedPropertyId {
        self.id
    }

    pub fn guid(&self) -> NamedPropertyGuid {
        self.guid
    }

    pub fn prop_id(&self) -> u16 {
        self.prop_index.0
    }

    pub fn hash_value(&self) -> u32 {
        let guid = u16::from(self.guid);
        let (prop_id, guid_index) = match self.id {
            NamedPropertyId::Number(id) => (id, guid << 1),
            NamedPropertyId::StringOffset(offset) => (offset, (guid << 1) | 0x0001),
        };

        prop_id ^ u32::from(guid_index)
    }
}

impl NamedPropReadWrite for NameIdEntry {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let prop_id = f.read_u32::<LittleEndian>()?;
        let guid_index = f.read_u16::<LittleEndian>()?;
        let prop_index = NamedPropertyIndex::try_from(f.read_u16::<LittleEndian>()?)?;

        let id = if guid_index & 0x0001 == 0 {
            NamedPropertyId::Number(prop_id)
        } else {
            NamedPropertyId::StringOffset(prop_id)
        };
        let guid_index = guid_index >> 1;
        let guid = NamedPropertyGuid::try_from(guid_index)?;

        Ok(Self {
            id,
            guid,
            prop_index,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        let guid = u16::from(self.guid);
        let (prop_id, guid_index) = match self.id {
            NamedPropertyId::Number(id) => (id, guid << 1),
            NamedPropertyId::StringOffset(offset) => (offset, (guid << 1) | 0x0001),
        };
        let prop_index = u16::from(self.prop_index);

        f.write_u32::<LittleEndian>(prop_id)?;
        f.write_u16::<LittleEndian>(guid_index)?;
        f.write_u16::<LittleEndian>(prop_index)
    }
}

#[derive(Default, Debug)]
pub struct NamedPropertyMapProperties {
    properties: BTreeMap<u16, PropertyValue>,
}

impl NamedPropertyMapProperties {
    pub fn get(&self, id: u16) -> Option<&PropertyValue> {
        self.properties.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u16, &PropertyValue)> {
        self.properties.iter()
    }

    pub fn hash_entry(&self, entry: NameIdEntry) -> io::Result<NameIdEntry> {
        Ok(match &entry.id {
            NamedPropertyId::Number(_) => entry,
            NamedPropertyId::StringOffset(offset) => {
                let string_entry = self.lookup_string(*offset)?;
                let hash_value =
                    NamedPropertyId::StringOffset(compute_crc(0, string_entry.buffer()));
                NameIdEntry::new(hash_value, NamedPropertyGuid::None, entry.prop_index)
            }
        })
    }

    pub fn bucket_count(&self) -> io::Result<u16> {
        let bucket_count = self
            .properties
            .get(&0x0001)
            .ok_or(MessagingError::NamedPropertyMapBucketCountNotFound)?;

        match bucket_count {
            PropertyValue::Integer32(value) => {
                let count = u16::try_from(*value)
                    .map_err(|_| MessagingError::NamedPropertyMapBucketCountOutOfBounds(*value))?;
                if count > u16::MAX - 0x1000 {
                    Err(MessagingError::NamedPropertyMapBucketCountOutOfBounds(*value).into())
                } else {
                    Ok(count)
                }
            }
            invalid => Err(
                MessagingError::InvalidNamedPropertyMapBucketCount(PropertyType::from(invalid))
                    .into(),
            ),
        }
    }

    pub fn hash_bucket(&self, name_id: &NameIdEntry) -> io::Result<Vec<NameIdEntry>> {
        let bucket_count = self.bucket_count()?;
        let bucket_offset = name_id.hash_value() % u32::from(bucket_count);
        let bucket_offset = u16::try_from(bucket_offset)
            .map_err(|_| MessagingError::NamedPropertyMapBucketOffsetOutOfBounds(bucket_offset))?;
        if bucket_offset > u16::MAX - 0x1000 {
            return Err(MessagingError::NamedPropertyMapBucketNotFound(bucket_offset).into());
        }
        let bucket_prop = 0x1000 + bucket_offset;

        let hash_bucket = self.properties.get(&bucket_prop).ok_or(
            MessagingError::NamedPropertyMapBucketNotFound(bucket_offset),
        )?;

        match hash_bucket {
            PropertyValue::Binary(value) => {
                let mut results = Vec::with_capacity(value.buffer().len() / 8);
                let mut cursor = Cursor::new(value.buffer());
                while let Ok(value) = NameIdEntry::read(&mut cursor) {
                    results.push(value);
                }
                Ok(results)
            }
            invalid => Err(MessagingError::InvalidNamedPropertyMapStreamString(
                PropertyType::from(invalid),
            )
            .into()),
        }
    }

    pub fn lookup_guid(&self, index: NamedPropertyGuid) -> io::Result<GuidValue> {
        let stream_guid = self
            .properties
            .get(&0x0002)
            .ok_or(MessagingError::NamedPropertyMapStreamGuidNotFound)?;

        match stream_guid {
            PropertyValue::Binary(value) => {
                let start = u16::from(index) as usize * 16;
                let end = start + 16;
                let mut cursor = Cursor::new(&value.buffer()[start..end]);
                let entry = PropertyValue::read(&mut cursor, PropertyType::Guid)?;
                match entry {
                    PropertyValue::Guid(guid) => Ok(guid),
                    invalid => Err(MessagingError::InvalidNamedPropertyMapStreamGuid(
                        PropertyType::from(&invalid),
                    )
                    .into()),
                }
            }
            invalid => Err(
                MessagingError::InvalidNamedPropertyMapStreamGuid(PropertyType::from(invalid))
                    .into(),
            ),
        }
    }

    pub fn stream_guid(&self) -> io::Result<Vec<GuidValue>> {
        let stream_guid = self
            .properties
            .get(&0x0002)
            .ok_or(MessagingError::NamedPropertyMapStreamGuidNotFound)?;

        match stream_guid {
            PropertyValue::Binary(value) => {
                let mut results = Vec::with_capacity(value.buffer().len() / 16);
                let mut cursor = Cursor::new(value.buffer());
                while let Ok(value) = PropertyValue::read(&mut cursor, PropertyType::Guid) {
                    match value {
                        PropertyValue::Guid(guid) => results.push(guid),
                        invalid => {
                            return Err(MessagingError::InvalidNamedPropertyMapStreamGuid(
                                PropertyType::from(&invalid),
                            )
                            .into())
                        }
                    }
                }
                Ok(results)
            }
            invalid => Err(
                MessagingError::InvalidNamedPropertyMapStreamGuid(PropertyType::from(invalid))
                    .into(),
            ),
        }
    }

    pub fn stream_entry(&self) -> io::Result<Vec<NameIdEntry>> {
        let stream_entry = self
            .properties
            .get(&0x0003)
            .ok_or(MessagingError::NamedPropertyMapStreamEntryNotFound)?;

        match stream_entry {
            PropertyValue::Binary(value) => {
                let mut results = Vec::with_capacity(value.buffer().len() / 8);
                let mut cursor = Cursor::new(value.buffer());
                while let Ok(value) = NameIdEntry::read(&mut cursor) {
                    results.push(value);
                }
                Ok(results)
            }
            invalid => Err(
                MessagingError::InvalidNamedPropertyMapStreamEntry(PropertyType::from(invalid))
                    .into(),
            ),
        }
    }

    pub fn lookup_string(&self, offset: u32) -> io::Result<StringEntry> {
        let stream_string = self
            .properties
            .get(&0x0004)
            .ok_or(MessagingError::NamedPropertyMapStreamStringNotFound)?;

        match stream_string {
            PropertyValue::Binary(value) => {
                let mut cursor = Cursor::new(&value.buffer()[offset as usize..]);
                let entry = StringEntry::read(&mut cursor)?;
                Ok(entry)
            }
            invalid => Err(MessagingError::InvalidNamedPropertyMapStreamString(
                PropertyType::from(invalid),
            )
            .into()),
        }
    }

    pub fn stream_string(&self) -> io::Result<Vec<StringEntry>> {
        let stream_string = self
            .properties
            .get(&0x0004)
            .ok_or(MessagingError::NamedPropertyMapStreamStringNotFound)?;

        match stream_string {
            PropertyValue::Binary(value) => {
                let mut results = Vec::new();
                let mut cursor = Cursor::new(value.buffer());
                while let Ok(value) = StringEntry::read(&mut cursor) {
                    let padding = ((i64::from(value.size()) + 3) / 4) * 4;
                    results.push(value);

                    if padding > 0 && cursor.seek(SeekFrom::Current(padding)).is_err() {
                        break;
                    }
                }
                Ok(results)
            }
            invalid => Err(MessagingError::InvalidNamedPropertyMapStreamString(
                PropertyType::from(invalid),
            )
            .into()),
        }
    }
}

pub struct UnicodeNamedPropertyMap {
    store: Arc<UnicodeStore>,
    properties: NamedPropertyMapProperties,
}

impl UnicodeNamedPropertyMap {
    pub fn store(&self) -> &Arc<UnicodeStore> {
        &self.store
    }

    pub fn read(store: Arc<UnicodeStore>) -> io::Result<Arc<Self>> {
        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let properties = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = UnicodeNodeBTree::read(file, *root.node_btree())?;
            let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u64::from(u32::from(NID_NAME_TO_ID_MAP)))?;
            let data = node.data();
            let block = block_btree.find_entry(file, u64::from(data))?;
            let heap = UnicodeHeapNode::new(UnicodeDataTree::read(file, encoding, &block)?);
            let header = heap.header(file, encoding, &block_btree)?;

            let tree = UnicodeHeapTree::new(heap, header.user_root());
            let prop_context = UnicodePropertyContext::new(node, tree);
            let properties = prop_context
                .properties(file, encoding, &block_btree)?
                .into_iter()
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()?;
            NamedPropertyMapProperties { properties }
        };

        Ok(Arc::new(Self { store, properties }))
    }

    pub fn properties(&self) -> &NamedPropertyMapProperties {
        &self.properties
    }
}

pub struct AnsiNamedPropertyMap {
    store: Arc<AnsiStore>,
    properties: NamedPropertyMapProperties,
}

impl AnsiNamedPropertyMap {
    pub fn store(&self) -> &Arc<AnsiStore> {
        &self.store
    }

    pub fn read(store: Arc<AnsiStore>) -> io::Result<Arc<Self>> {
        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let properties = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = AnsiNodeBTree::read(file, *root.node_btree())?;
            let block_btree = AnsiBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u32::from(NID_NAME_TO_ID_MAP))?;
            let data = node.data();
            let block = block_btree.find_entry(file, u32::from(data))?;
            let heap = AnsiHeapNode::new(AnsiDataTree::read(file, encoding, &block)?);
            let header = heap.header(file, encoding, &block_btree)?;

            let tree = AnsiHeapTree::new(heap, header.user_root());
            let prop_context = AnsiPropertyContext::new(node, tree);
            let properties = prop_context
                .properties(file, encoding, &block_btree)?
                .into_iter()
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()?;
            NamedPropertyMapProperties { properties }
        };

        Ok(Arc::new(Self { store, properties }))
    }

    pub fn properties(&self) -> &NamedPropertyMapProperties {
        &self.properties
    }
}
