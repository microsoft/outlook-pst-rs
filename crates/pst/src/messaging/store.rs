//! ## [Message Store](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/aa0539bd-e7bf-4cec-8bde-0b87c2a86baf)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::{
    cell::OnceCell,
    collections::BTreeMap,
    fmt::Debug,
    io::{self, Read, Write},
    rc::{Rc, Weak},
};

use super::{folder::*, message::*, read_write::*, *};
use crate::{
    ltp::{
        heap::HeapNode,
        prop_context::{PropertyContext, PropertyValue},
        prop_type::PropertyType,
        read_write::*,
        table_context::TableContext,
    },
    ndb::{
        block_id::BlockId,
        header::Header,
        node_id::{NodeId, NodeIdType, NID_MESSAGE_STORE, NID_ROOT_FOLDER},
        page::*,
        read_write::*,
        root::Root,
    },
    *,
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

impl StoreKeyReadWrite for StoreRecordKey {
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

impl StoreKeyReadWrite for EntryId {
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

impl TryFrom<&[u8]> for EntryId {
    type Error = io::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let mut reader = value;
        EntryId::read(&mut reader)
    }
}

impl TryFrom<&EntryId> for Vec<u8> {
    type Error = io::Error;

    fn try_from(value: &EntryId) -> Result<Self, Self::Error> {
        let mut result = vec![];
        value.write(&mut result)?;
        Ok(result)
    }
}

#[derive(Default, Debug)]
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

    pub fn make_entry_id(&self, node_id: NodeId) -> io::Result<EntryId> {
        let record_key = self.record_key()?;
        Ok(EntryId::new(record_key, node_id))
    }

    pub fn matches_record_key(&self, entry_id: &EntryId) -> io::Result<bool> {
        let store_record_key = self.record_key()?;
        Ok(store_record_key == entry_id.record_key)
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
                MessagingError::InvalidStoreIpmSubTreeEntryId(PropertyType::from(invalid)).into(),
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
                MessagingError::InvalidStoreIpmWastebasketEntryId(PropertyType::from(invalid))
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
                Err(MessagingError::InvalidStoreFinderEntryId(PropertyType::from(invalid)).into())
            }
        }
    }
}

pub trait Store {
    fn properties(&self) -> &StoreProperties;
    fn root_hierarchy_table(&self) -> io::Result<Rc<dyn TableContext>>;
    fn unique_value(&self) -> u32;
    fn open_folder(&self, entry_id: &EntryId) -> io::Result<Rc<dyn Folder>>;
    fn open_message(
        &self,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<dyn Message>>;
    fn named_property_map(&self) -> io::Result<Rc<dyn NamedPropertyMap>>;
    fn search_update_queue(&self) -> io::Result<Rc<dyn SearchUpdateQueue>>;
}

struct StoreInner<Pst>
where
    Pst: PstFile + PstFileLock<Pst> + 'static,
{
    pst: Rc<Pst>,
    node_btree: PstFileReadWriteNodeBTree<Pst>,
    block_btree: PstFileReadWriteBlockBTree<Pst>,
    properties: StoreProperties,
    store: Weak<Pst::Store>,
    root_hierarchy_table: OnceCell<io::Result<Rc<dyn TableContext>>>,
}

impl<Pst> StoreInner<Pst>
where
    Pst: PstFile + PstFileLock<Pst>,
    <Pst as PstFile>::BTreeKey: BTreePageKeyReadWrite,
    <Pst as PstFile>::NodeBTreeEntry: NodeBTreeEntryReadWrite,
    <Pst as PstFile>::NodeBTree: RootBTreeReadWrite,
    <<Pst as PstFile>::NodeBTree as RootBTree>::IntermediatePage:
        RootBTreeIntermediatePageReadWrite<
            Pst,
            <Pst as PstFile>::NodeBTreeEntry,
            <<Pst as PstFile>::NodeBTree as RootBTree>::LeafPage,
        >,
    <<<Pst as PstFile>::NodeBTree as RootBTree>::IntermediatePage as BTreePage>::Entry:
        BTreePageEntryReadWrite,
    <<Pst as PstFile>::NodeBTree as RootBTree>::LeafPage: RootBTreeLeafPageReadWrite<Pst>,
    <Pst as PstFile>::BlockBTreeEntry: BlockBTreeEntryReadWrite,
    <Pst as PstFile>::BlockBTree: RootBTreeReadWrite,
    <<Pst as PstFile>::BlockBTree as RootBTree>::Entry: BTreeEntryReadWrite,
    <<Pst as PstFile>::BlockBTree as RootBTree>::IntermediatePage:
        RootBTreeIntermediatePageReadWrite<
            Pst,
            <<Pst as PstFile>::BlockBTree as RootBTree>::Entry,
            <<Pst as PstFile>::BlockBTree as RootBTree>::LeafPage,
        >,
    <<Pst as PstFile>::BlockBTree as RootBTree>::LeafPage:
        RootBTreeLeafPageReadWrite<Pst> + BTreePageReadWrite,
    <Pst as PstFile>::BlockTrailer: BlockTrailerReadWrite,
    <Pst as PstFile>::HeapNode: HeapNodeReadWrite<Pst>,
    <Pst as PstFile>::PropertyTree: HeapTreeReadWrite<Pst>,
    <Pst as PstFile>::TableContext: TableContextReadWrite<Pst>,
    <Pst as PstFile>::PropertyContext: PropertyContextReadWrite<Pst>,
    <Pst as PstFile>::Folder: FolderReadWrite<Pst>,
    <Pst as PstFile>::Message: MessageReadWrite<Pst>,
    <Pst as PstFile>::NamedPropertyMap: NamedPropertyMapReadWrite<Pst>,
    <Pst as PstFile>::SearchUpdateQueue: SearchUpdateQueueReadWrite<Pst>,
{
    fn read(pst: Rc<Pst>) -> io::Result<Self> {
        let header = pst.header();
        let root = header.root();

        let (node_btree, block_btree, properties) = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = <<Pst as PstFile>::NodeBTree as RootBTreeReadWrite>::read(
                file,
                *root.node_btree(),
            )?;
            let block_btree = <<Pst as PstFile>::BlockBTree as RootBTreeReadWrite>::read(
                file,
                *root.block_btree(),
            )?;

            let mut page_cache = pst.node_cache();
            let node_key: <Pst as PstFile>::BTreeKey = u32::from(NID_MESSAGE_STORE).into();
            let node = node_btree.find_entry(file, node_key, &mut page_cache)?;

            let mut page_cache = pst.block_cache();
            let data = node.data();
            let heap = <<Pst as PstFile>::HeapNode as HeapNodeReadWrite<Pst>>::read(
                file,
                &block_btree,
                &mut page_cache,
                encoding,
                data.search_key(),
            )?;
            let header = heap.header()?;

            let tree = <Pst as PstFile>::PropertyTree::new(heap, header.user_root());
            let prop_context = <Pst as PstFile>::PropertyContext::new(node, tree);
            let properties = prop_context
                .properties()?
                .into_iter()
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, &mut page_cache, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()?;
            let properties = StoreProperties { properties };

            (node_btree, block_btree, properties)
        };

        Ok(Self {
            pst,
            node_btree,
            block_btree,
            properties,
            store: Default::default(),
            root_hierarchy_table: Default::default(),
        })
    }

    fn root_hierarchy_table(&self) -> io::Result<Rc<dyn TableContext>> {
        let hierarchy_table =
            self.root_hierarchy_table
                .get_or_init(|| {
                    let store = self.store.upgrade().ok_or(
                        MessagingError::StoreRootHierarchyTableFailed(
                            "Store has been dropped".to_string(),
                        ),
                    )?;
                    let mut file = self
                        .pst
                        .reader()
                        .lock()
                        .map_err(|_| MessagingError::FailedToLockFile)?;

                    let file = &mut *file;
                    let node_id = NodeId::new(NodeIdType::HierarchyTable, NID_ROOT_FOLDER.index())?;
                    let mut page_cache = self.pst.node_cache();
                    let node_key: <Pst as PstFile>::BTreeKey = u32::from(node_id).into();
                    let node = self
                        .node_btree
                        .find_entry(file, node_key, &mut page_cache)?;

                    <<Pst as PstFile>::TableContext as TableContextReadWrite<Pst>>::read(
                        store.clone(),
                        node,
                    )
                })
                .as_ref()
                .map_err(|err| format!("{err:?}"))
                .cloned()
                .map_err(MessagingError::StoreRootHierarchyTableFailed)?;

        Ok(hierarchy_table)
    }

    fn open_folder(&self, entry_id: &EntryId) -> io::Result<Rc<dyn Folder>> {
        let store = self.store.upgrade().ok_or(MessagingError::StoreOpenFolder(
            "Store has been dropped".to_string(),
        ))?;
        Ok(<<Pst as PstFile>::Folder as FolderReadWrite<Pst>>::read(
            store, entry_id,
        )?)
    }

    fn open_message(
        &self,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<dyn Message>> {
        let store = self.store.upgrade().ok_or(MessagingError::StoreOpenFolder(
            "Store has been dropped".to_string(),
        ))?;
        Ok(<<Pst as PstFile>::Message as MessageReadWrite<Pst>>::read(
            store, entry_id, prop_ids,
        )?)
    }

    fn named_property_map(&self) -> io::Result<Rc<dyn NamedPropertyMap>> {
        let store = self
            .store
            .upgrade()
            .ok_or(MessagingError::StoreNamedPropertyMap(
                "Store has been dropped".to_string(),
            ))?;
        Ok(<<Pst as PstFile>::NamedPropertyMap as NamedPropertyMapReadWrite<Pst>>::read(store)?)
    }

    fn search_update_queue(&self) -> io::Result<Rc<dyn SearchUpdateQueue>> {
        let store = self
            .store
            .upgrade()
            .ok_or(MessagingError::StoreSearchUpdateQueue(
                "Store has been dropped".to_string(),
            ))?;
        Ok(<<Pst as PstFile>::SearchUpdateQueue as SearchUpdateQueueReadWrite<Pst>>::read(store)?)
    }

    fn unique_value(&self) -> u32 {
        self.pst.header().unique_value()
    }
}

pub struct UnicodeStore {
    inner: StoreInner<UnicodePstFile>,
}

impl UnicodeStore {
    pub fn read(pst: Rc<UnicodePstFile>) -> io::Result<Rc<Self>> {
        let inner = StoreInner::read(pst)?;
        Ok(Rc::new_cyclic(|store| Self::new_cyclic(inner, store)))
    }

    fn new_cyclic(inner: StoreInner<UnicodePstFile>, store: &Weak<Self>) -> Self {
        Self {
            inner: StoreInner {
                store: store.clone(),
                ..inner
            },
        }
    }
}

impl Store for UnicodeStore {
    fn properties(&self) -> &StoreProperties {
        &self.inner.properties
    }

    fn root_hierarchy_table(&self) -> io::Result<Rc<dyn TableContext>> {
        self.inner.root_hierarchy_table()
    }

    fn unique_value(&self) -> u32 {
        self.inner.unique_value()
    }

    fn open_folder(&self, entry_id: &EntryId) -> io::Result<Rc<dyn Folder>> {
        self.inner.open_folder(entry_id)
    }

    fn open_message(
        &self,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<dyn Message>> {
        self.inner.open_message(entry_id, prop_ids)
    }

    fn named_property_map(&self) -> io::Result<Rc<dyn NamedPropertyMap>> {
        self.inner.named_property_map()
    }

    fn search_update_queue(&self) -> io::Result<Rc<dyn SearchUpdateQueue>> {
        self.inner.search_update_queue()
    }
}

impl StoreReadWrite<UnicodePstFile> for UnicodeStore {
    fn pst(&self) -> &UnicodePstFile {
        self.inner.pst.as_ref()
    }

    fn node_btree(&self) -> &PstFileReadWriteNodeBTree<UnicodePstFile> {
        &self.inner.node_btree
    }

    fn block_btree(&self) -> &PstFileReadWriteBlockBTree<UnicodePstFile> {
        &self.inner.block_btree
    }
}

pub struct AnsiStore {
    inner: StoreInner<AnsiPstFile>,
}

impl AnsiStore {
    pub fn read(pst: Rc<AnsiPstFile>) -> io::Result<Rc<Self>> {
        let inner = StoreInner::read(pst)?;
        Ok(Rc::new_cyclic(|store| Self::new_cyclic(inner, store)))
    }

    fn new_cyclic(inner: StoreInner<AnsiPstFile>, store: &Weak<Self>) -> Self {
        Self {
            inner: StoreInner {
                store: store.clone(),
                ..inner
            },
        }
    }
}

impl Store for AnsiStore {
    fn properties(&self) -> &StoreProperties {
        &self.inner.properties
    }

    fn root_hierarchy_table(&self) -> io::Result<Rc<dyn TableContext>> {
        self.inner.root_hierarchy_table()
    }

    fn unique_value(&self) -> u32 {
        self.inner.unique_value()
    }

    fn open_folder(&self, entry_id: &EntryId) -> io::Result<Rc<dyn Folder>> {
        self.inner.open_folder(entry_id)
    }

    fn open_message(
        &self,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<dyn Message>> {
        self.inner.open_message(entry_id, prop_ids)
    }

    fn named_property_map(&self) -> io::Result<Rc<dyn NamedPropertyMap>> {
        self.inner.named_property_map()
    }

    fn search_update_queue(&self) -> io::Result<Rc<dyn SearchUpdateQueue>> {
        self.inner.search_update_queue()
    }
}

impl StoreReadWrite<AnsiPstFile> for AnsiStore {
    fn pst(&self) -> &AnsiPstFile {
        self.inner.pst.as_ref()
    }

    fn node_btree(&self) -> &PstFileReadWriteNodeBTree<AnsiPstFile> {
        &self.inner.node_btree
    }

    fn block_btree(&self) -> &PstFileReadWriteBlockBTree<AnsiPstFile> {
        &self.inner.block_btree
    }
}
