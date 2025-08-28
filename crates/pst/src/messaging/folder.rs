//! ## [Folders](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/dee5b9d0-5513-4c5e-94aa-8bd28a9350b2)

use std::{cell::OnceCell, collections::BTreeMap, io, rc::Rc};

use super::{read_write::*, store::*, *};
use crate::{
    ltp::{
        heap::HeapNode,
        prop_context::{BinaryValue, PropertyContext, PropertyValue},
        prop_type::PropertyType,
        read_write::*,
        table_context::TableContext,
    },
    ndb::{
        block_id::BlockId,
        header::Header,
        node_id::{NodeId, NodeIdType, NID_ROOT_FOLDER},
        page::{BTreePage, NodeBTreeEntry, RootBTree},
        read_write::*,
        root::Root,
    },
    AnsiPstFile, PstFile, PstFileLock, UnicodePstFile,
};

#[derive(Default, Debug)]
pub struct FolderProperties {
    node_id: NodeId,
    properties: BTreeMap<u16, PropertyValue>,
}

impl FolderProperties {
    pub fn get(&self, id: u16) -> Option<&PropertyValue> {
        self.properties.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u16, &PropertyValue)> {
        self.properties.iter()
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn display_name(&self) -> io::Result<String> {
        let display_name = self
            .properties
            .get(&0x3001)
            .ok_or(MessagingError::FolderDisplayNameNotFound)?;

        match display_name {
            PropertyValue::String8(value) => Ok(value.to_string()),
            PropertyValue::Unicode(value) => Ok(value.to_string()),
            invalid => {
                Err(MessagingError::InvalidFolderDisplayName(PropertyType::from(invalid)).into())
            }
        }
    }

    pub fn content_count(&self) -> io::Result<i32> {
        let content_count = self
            .properties
            .get(&0x3602)
            .ok_or(MessagingError::FolderContentCountNotFound)?;

        match content_count {
            PropertyValue::Integer32(value) => Ok(*value),
            invalid => {
                Err(MessagingError::InvalidFolderContentCount(PropertyType::from(invalid)).into())
            }
        }
    }

    pub fn unread_count(&self) -> io::Result<i32> {
        let unread_count = self
            .properties
            .get(&0x3603)
            .ok_or(MessagingError::FolderUnreadCountNotFound)?;

        match unread_count {
            PropertyValue::Integer32(value) => Ok(*value),
            invalid => {
                Err(MessagingError::InvalidFolderUnreadCount(PropertyType::from(invalid)).into())
            }
        }
    }

    pub fn has_sub_folders(&self) -> io::Result<bool> {
        let entry_id = self
            .properties
            .get(&0x360A)
            .ok_or(MessagingError::FolderHasSubfoldersNotFound)?;

        match entry_id {
            PropertyValue::Boolean(value) => Ok(*value),
            invalid => {
                Err(MessagingError::InvalidFolderHasSubfolders(PropertyType::from(invalid)).into())
            }
        }
    }
}

pub trait Folder {
    fn store(&self) -> Rc<dyn Store>;
    fn properties(&self) -> &FolderProperties;
    fn hierarchy_table(&self) -> Option<&Rc<dyn TableContext>>;
    fn contents_table(&self) -> Option<&Rc<dyn TableContext>>;
    fn associated_table(&self) -> Option<&Rc<dyn TableContext>>;
}

struct FolderInner<Pst>
where
    Pst: PstFile,
{
    store: Rc<Pst::Store>,
    properties: FolderProperties,
    hierarchy_table: OnceCell<Option<Rc<dyn TableContext>>>,
    contents_table: OnceCell<Option<Rc<dyn TableContext>>>,
    associated_table: OnceCell<Option<Rc<dyn TableContext>>>,
}

impl<Pst> FolderInner<Pst>
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
    <Pst as PstFile>::Store: StoreReadWrite<Pst>,
{
    fn read(store: Rc<<Pst as PstFile>::Store>, entry_id: &EntryId) -> io::Result<Self> {
        let node_id = entry_id.node_id();
        let node_id_type = node_id.id_type()?;
        match node_id_type {
            NodeIdType::NormalFolder | NodeIdType::SearchFolder => {}
            _ => {
                return Err(MessagingError::InvalidFolderEntryIdType(node_id_type).into());
            }
        }
        if !store.properties().matches_record_key(entry_id)? {
            return Err(MessagingError::EntryIdWrongStore.into());
        }

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
            let node_btree = <<Pst as PstFile>::NodeBTree as RootBTreeReadWrite>::read(
                file,
                *root.node_btree(),
            )?;
            let block_btree = <<Pst as PstFile>::BlockBTree as RootBTreeReadWrite>::read(
                file,
                *root.block_btree(),
            )?;

            let mut node_page_cache = pst.node_cache();
            let node_key: <Pst as PstFile>::BTreeKey = u32::from(node_id).into();
            let node = node_btree.find_entry(file, node_key, &mut node_page_cache)?;
            let mut block_page_cache = pst.block_cache();
            let data = node.data();
            let heap = <<Pst as PstFile>::HeapNode as HeapNodeReadWrite<Pst>>::read(
                file,
                &block_btree,
                &mut block_page_cache,
                encoding,
                data.search_key(),
            )?;
            let header = heap.header()?;

            let tree = <Pst as PstFile>::PropertyTree::new(heap, header.user_root());
            let prop_context = <Pst as PstFile>::PropertyContext::new(node, tree);
            let folder_type = if entry_id.node_id() == NID_ROOT_FOLDER {
                0
            } else if node_id_type == NodeIdType::SearchFolder {
                2
            } else {
                1
            };
            let entry_id = entry_id.try_into()?;
            let properties = prop_context
                .properties()?
                .into_iter()
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, &mut block_page_cache, record)
                        .map(|value| (prop_id, value))
                })
                .chain([
                    Ok((0x0FFF, PropertyValue::Binary(BinaryValue::new(entry_id)))),
                    Ok((0x3601, PropertyValue::Integer32(folder_type))),
                ])
                .collect::<io::Result<BTreeMap<_, _>>>()?;

            FolderProperties {
                node_id,
                properties,
            }
        };

        Ok(Self {
            store,
            properties,
            hierarchy_table: Default::default(),
            contents_table: Default::default(),
            associated_table: Default::default(),
        })
    }

    fn read_table(&self, node_id_type: NodeIdType) -> io::Result<Option<Rc<dyn TableContext>>> {
        let pst = self.store.pst();
        let header = pst.header();
        let root = header.root();

        let node = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let node_btree = <<Pst as PstFile>::NodeBTree as RootBTreeReadWrite>::read(
                file,
                *root.node_btree(),
            )?;

            let node_id = NodeId::new(node_id_type, self.properties.node_id.index())?;
            let node_key: <Pst as PstFile>::BTreeKey = u32::from(node_id).into();
            let mut node_page_cache = pst.node_cache();

            let Ok(node) = node_btree.find_entry(file, node_key, &mut node_page_cache) else {
                return Ok(None);
            };
            node
        };

        Ok(Some(
            <<Pst as PstFile>::TableContext as TableContextReadWrite<Pst>>::read(
                self.store.clone(),
                node,
            )?,
        ))
    }

    fn hierarchy_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.hierarchy_table
            .get_or_init(|| self.read_table(NodeIdType::HierarchyTable).ok()?)
            .as_ref()
    }

    fn contents_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.contents_table
            .get_or_init(|| self.read_table(NodeIdType::ContentsTable).ok()?)
            .as_ref()
    }

    fn associated_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.associated_table
            .get_or_init(|| self.read_table(NodeIdType::AssociatedContentsTable).ok()?)
            .as_ref()
    }
}

pub struct UnicodeFolder {
    inner: FolderInner<UnicodePstFile>,
}

impl UnicodeFolder {
    pub fn read(store: Rc<UnicodeStore>, entry_id: &EntryId) -> io::Result<Rc<Self>> {
        <Self as FolderReadWrite<UnicodePstFile>>::read(store, entry_id)
    }
}

impl Folder for UnicodeFolder {
    fn store(&self) -> Rc<dyn Store> {
        self.inner.store.clone()
    }

    fn properties(&self) -> &FolderProperties {
        &self.inner.properties
    }

    fn hierarchy_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.hierarchy_table()
    }

    fn contents_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.contents_table()
    }

    fn associated_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.associated_table()
    }
}

impl FolderReadWrite<UnicodePstFile> for UnicodeFolder {
    fn read(store: Rc<UnicodeStore>, entry_id: &EntryId) -> io::Result<Rc<Self>> {
        let inner = FolderInner::read(store, entry_id)?;
        Ok(Rc::new(Self { inner }))
    }
}

pub struct AnsiFolder {
    inner: FolderInner<AnsiPstFile>,
}

impl AnsiFolder {
    pub fn read(store: Rc<AnsiStore>, entry_id: &EntryId) -> io::Result<Rc<Self>> {
        <Self as FolderReadWrite<AnsiPstFile>>::read(store, entry_id)
    }
}

impl Folder for AnsiFolder {
    fn store(&self) -> Rc<dyn Store> {
        self.inner.store.clone()
    }

    fn properties(&self) -> &FolderProperties {
        &self.inner.properties
    }

    fn hierarchy_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.hierarchy_table()
    }

    fn contents_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.contents_table()
    }

    fn associated_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.associated_table()
    }
}

impl FolderReadWrite<AnsiPstFile> for AnsiFolder {
    fn read(store: Rc<AnsiStore>, entry_id: &EntryId) -> io::Result<Rc<Self>> {
        let inner = FolderInner::read(store, entry_id)?;
        Ok(Rc::new(Self { inner }))
    }
}
