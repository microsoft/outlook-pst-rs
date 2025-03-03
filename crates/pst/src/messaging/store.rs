//! ## [Message Store](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/aa0539bd-e7bf-4cec-8bde-0b87c2a86baf)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::{
    collections::BTreeMap,
    fmt::Debug,
    io::{self, Read, Write},
};

use super::{read_write::*, *};
use crate::{
    ltp::{
        heap::{AnsiHeapNode, UnicodeHeapNode},
        prop_context::{AnsiPropertyContext, PropertyValue, UnicodePropertyContext},
        prop_type::PropertyType,
        tree::{AnsiHeapTree, UnicodeHeapTree},
    },
    ndb::{
        block::{AnsiDataTree, UnicodeDataTree},
        header::Header,
        node_id::{NodeId, NID_MESSAGE_STORE},
        page::{
            AnsiBlockBTree, AnsiNodeBTree, NodeBTreeEntry, RootBTree, UnicodeBlockBTree,
            UnicodeNodeBTree,
        },
        read_write::NodeIdReadWrite,
        root::Root,
    },
    AnsiPstFile, UnicodePstFile,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StoreRecordKey {
    record_key: [u8; 16],
}

impl StoreRecordKey {
    pub fn new(record_key: [u8; 16]) -> Self {
        Self { record_key }
    }

    pub fn record_key(&self) -> &[u8; 16] {
        &self.record_key
    }
}

impl StoreReadWrite for StoreRecordKey {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let mut record_key = [0; 16];
        f.read_exact(&mut record_key)?;
        Ok(Self::new(record_key))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_all(&self.record_key)
    }
}

impl Debug for StoreRecordKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self
            .record_key
            .iter()
            .map(|ch| format!("{ch:02X}"))
            .collect::<Vec<_>>()
            .join("-");
        write!(f, "{value}")
    }
}

impl TryFrom<&[u8]> for StoreRecordKey {
    type Error = MessagingError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != 16 {
            return Err(MessagingError::InvalidStoreRecordKeySize(value.len()));
        }

        let mut record_key = [0; 16];
        record_key.copy_from_slice(value);
        Ok(Self::new(record_key))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EntryId {
    record_key: StoreRecordKey,
    node_id: NodeId,
}

impl EntryId {
    pub fn new(record_key: StoreRecordKey, node_id: NodeId) -> Self {
        Self {
            record_key,
            node_id,
        }
    }

    pub fn record_key(&self) -> &[u8; 16] {
        self.record_key.record_key()
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
}

impl StoreReadWrite for EntryId {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        // rgbFlags
        let flags = f.read_u32::<LittleEndian>()?;
        if flags != 0 {
            return Err(MessagingError::InvalidEntryIdFlags(flags).into());
        }

        // uid
        let record_key = StoreRecordKey::read(f)?;

        // nid
        let node_id = NodeId::read(f)?;

        Ok(Self::new(record_key, node_id))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        // rgbFlags
        f.write_u32::<LittleEndian>(0)?;

        // uid
        self.record_key.write(f)?;

        // nid
        self.node_id.write(f)
    }
}

impl From<&EntryId> for NodeId {
    fn from(value: &EntryId) -> Self {
        value.node_id
    }
}

pub struct StoreProperties {
    properties: BTreeMap<u16, PropertyValue>,
}

impl StoreProperties {
    pub fn get(&self, id: u16) -> Option<&PropertyValue> {
        self.properties.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u16, &PropertyValue)> {
        self.properties.iter()
    }

    pub fn record_key(&self) -> io::Result<StoreRecordKey> {
        let record_key = self
            .properties
            .get(&0xFF9)
            .ok_or(MessagingError::StoreRecordKeyNotFound)?;

        match record_key {
            PropertyValue::Binary(value) => Ok(StoreRecordKey::try_from(value.buffer())?),
            invalid => {
                Err(MessagingError::InvalidStoreRecordKey(PropertyType::from(invalid)).into())
            }
        }
    }

    pub fn display_name(&self) -> io::Result<String> {
        let display_name = self
            .properties
            .get(&0x3001)
            .ok_or(MessagingError::StoreDisplayNameNotFound)?;

        match display_name {
            PropertyValue::String8(value) => Ok(value.to_string()),
            PropertyValue::Unicode(value) => Ok(value.to_string()),
            invalid => {
                Err(MessagingError::InvalidStoreDisplayName(PropertyType::from(invalid)).into())
            }
        }
    }

    pub fn ipm_sub_tree_entry_id(&self) -> io::Result<EntryId> {
        let entry_id = self
            .properties
            .get(&0x35E0)
            .ok_or(MessagingError::StoreIpmSubTreeEntryIdNotFound)?;

        match entry_id {
            PropertyValue::Binary(value) => EntryId::read(&mut value.buffer()),
            invalid => Err(
                MessagingError::StoreInvalidIpmSubTreeEntryId(PropertyType::from(invalid)).into(),
            ),
        }
    }

    pub fn ipm_wastebasket_entry_id(&self) -> io::Result<EntryId> {
        let entry_id = self
            .properties
            .get(&0x35E3)
            .ok_or(MessagingError::StoreIpmWastebasketEntryIdNotFound)?;

        match entry_id {
            PropertyValue::Binary(value) => EntryId::read(&mut value.buffer()),
            invalid => Err(
                MessagingError::StoreInvalidIpmWastebasketEntryId(PropertyType::from(invalid))
                    .into(),
            ),
        }
    }

    pub fn finder_entry_id(&self) -> io::Result<EntryId> {
        let entry_id = self
            .properties
            .get(&0x35E7)
            .ok_or(MessagingError::StoreFinderEntryIdNotFound)?;

        match entry_id {
            PropertyValue::Binary(value) => EntryId::read(&mut value.buffer()),
            invalid => {
                Err(MessagingError::StoreInvalidFinderEntryId(PropertyType::from(invalid)).into())
            }
        }
    }
}

pub struct UnicodeStore<'a> {
    pst: &'a UnicodePstFile,
    properties: StoreProperties,
}

impl<'a> UnicodeStore<'a> {
    pub fn pst(&self) -> &UnicodePstFile {
        self.pst
    }

    pub fn read(pst: &'a UnicodePstFile) -> io::Result<Self> {
        let header = pst.header();
        let root = header.root();

        let properties = {
            let mut file = pst
                .file()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = UnicodeNodeBTree::read(file, *root.node_btree())?;
            let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u64::from(u32::from(NID_MESSAGE_STORE)))?;
            let data = node.data();
            let block = block_btree.find_entry(file, u64::from(data))?;
            let heap = UnicodeHeapNode::new(UnicodeDataTree::read(file, encoding, &block)?);
            let header = heap.header(file, encoding, &block_btree)?;

            let tree = UnicodeHeapTree::new(heap, header.user_root());
            let prop_context = UnicodePropertyContext::new(node, tree);
            prop_context
                .properties(file, encoding, &block_btree)?
                .into_iter()
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()
        }?;
        let properties = StoreProperties { properties };

        Ok(Self { pst, properties })
    }

    pub fn properties(&self) -> &StoreProperties {
        &self.properties
    }

    pub fn make_entry_id(&self, node_id: NodeId) -> io::Result<EntryId> {
        let record_key = self.properties.record_key()?;
        Ok(EntryId::new(record_key, node_id))
    }

    pub fn matches_record_key(&self, entry_id: &EntryId) -> io::Result<bool> {
        let store_record_key = self.properties.record_key()?;
        Ok(store_record_key == entry_id.record_key)
    }
}

pub struct AnsiStore<'a> {
    pst: &'a AnsiPstFile,
    properties: StoreProperties,
}

impl<'a> AnsiStore<'a> {
    pub fn pst(&self) -> &AnsiPstFile {
        self.pst
    }

    pub fn read(pst: &'a AnsiPstFile) -> io::Result<Self> {
        let header = pst.header();
        let root = header.root();

        let properties = {
            let mut file = pst
                .file()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = AnsiNodeBTree::read(file, *root.node_btree())?;
            let block_btree = AnsiBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u32::from(NID_MESSAGE_STORE))?;
            let data = node.data();
            let block = block_btree.find_entry(file, u32::from(data))?;
            let heap = AnsiHeapNode::new(AnsiDataTree::read(file, encoding, &block)?);
            let header = heap.header(file, encoding, &block_btree)?;

            let tree = AnsiHeapTree::new(heap, header.user_root());
            let prop_context = AnsiPropertyContext::new(node, tree);
            prop_context
                .properties(file, encoding, &block_btree)?
                .into_iter()
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()
        }?;
        let properties = StoreProperties { properties };

        Ok(Self { pst, properties })
    }

    pub fn properties(&self) -> &StoreProperties {
        &self.properties
    }

    pub fn make_entry_id(&self, node_id: NodeId) -> io::Result<EntryId> {
        let record_key = self.properties.record_key()?;
        Ok(EntryId::new(record_key, node_id))
    }

    pub fn matches_record_key(&self, entry_id: &EntryId) -> io::Result<bool> {
        let store_record_key = self.properties.record_key()?;
        Ok(store_record_key == entry_id.record_key)
    }
}
