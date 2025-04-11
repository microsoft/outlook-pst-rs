//! ## [Folders](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/dee5b9d0-5513-4c5e-94aa-8bd28a9350b2)

use std::{collections::BTreeMap, io, rc::Rc};

use super::{store::*, *};
use crate::{
    ltp::{
        heap::{AnsiHeapNode, UnicodeHeapNode},
        prop_context::{AnsiPropertyContext, PropertyValue, UnicodePropertyContext},
        prop_type::PropertyType,
        table_context::{AnsiTableContext, UnicodeTableContext},
        tree::{AnsiHeapTree, UnicodeHeapTree},
    },
    ndb::{
        block::{AnsiDataTree, UnicodeDataTree},
        header::Header,
        node_id::{NodeId, NodeIdType},
        page::{
            AnsiBlockBTree, AnsiNodeBTree, NodeBTreeEntry, UnicodeBlockBTree, UnicodeNodeBTree,
        },
        root::Root,
    },
    PstFile,
};

#[derive(Default, Debug)]
pub struct FolderProperties {
    properties: BTreeMap<u16, PropertyValue>,
}

impl FolderProperties {
    pub fn get(&self, id: u16) -> Option<&PropertyValue> {
        self.properties.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u16, &PropertyValue)> {
        self.properties.iter()
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

pub struct UnicodeFolder {
    store: Rc<UnicodeStore>,
    properties: FolderProperties,
    hierarchy_table: Option<UnicodeTableContext>,
    contents_table: Option<UnicodeTableContext>,
    associated_table: Option<UnicodeTableContext>,
}

impl UnicodeFolder {
    pub fn store(&self) -> &Rc<UnicodeStore> {
        &self.store
    }

    pub fn read(store: Rc<UnicodeStore>, entry_id: &EntryId) -> io::Result<Rc<Self>> {
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

        let (properties, hierarchy_table, contents_table, associated_table) = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = UnicodeNodeBTree::read(file, *root.node_btree())?;
            let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u64::from(u32::from(node_id)))?;
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
            let properties = FolderProperties { properties };

            let node_id = NodeId::new(NodeIdType::HierarchyTable, node_id.index())?;
            let hierarchy_table =
                if let Ok(node) = node_btree.find_entry(file, u64::from(u32::from(node_id))) {
                    Some(UnicodeTableContext::read(
                        file,
                        encoding,
                        &block_btree,
                        node,
                    )?)
                } else {
                    None
                };

            let node_id = NodeId::new(NodeIdType::ContentsTable, node_id.index())?;
            let contents_table =
                if let Ok(node) = node_btree.find_entry(file, u64::from(u32::from(node_id))) {
                    Some(UnicodeTableContext::read(
                        file,
                        encoding,
                        &block_btree,
                        node,
                    )?)
                } else {
                    None
                };

            let node_id = NodeId::new(NodeIdType::AssociatedContentsTable, node_id.index())?;
            let associated_table =
                if let Ok(node) = node_btree.find_entry(file, u64::from(u32::from(node_id))) {
                    Some(UnicodeTableContext::read(
                        file,
                        encoding,
                        &block_btree,
                        node,
                    )?)
                } else {
                    None
                };

            (
                properties,
                hierarchy_table,
                contents_table,
                associated_table,
            )
        };

        Ok(Rc::new(Self {
            store,
            properties,
            hierarchy_table,
            contents_table,
            associated_table,
        }))
    }

    pub fn properties(&self) -> &FolderProperties {
        &self.properties
    }

    pub fn hierarchy_table(&self) -> Option<&UnicodeTableContext> {
        self.hierarchy_table.as_ref()
    }

    pub fn contents_table(&self) -> Option<&UnicodeTableContext> {
        self.contents_table.as_ref()
    }

    pub fn associated_table(&self) -> Option<&UnicodeTableContext> {
        self.associated_table.as_ref()
    }
}

pub struct AnsiFolder {
    store: Rc<AnsiStore>,
    properties: FolderProperties,
    hierarchy_table: Option<AnsiTableContext>,
    contents_table: Option<AnsiTableContext>,
    associated_table: Option<AnsiTableContext>,
}

impl AnsiFolder {
    pub fn store(&self) -> &Rc<AnsiStore> {
        &self.store
    }

    pub fn read(store: Rc<AnsiStore>, entry_id: &EntryId) -> io::Result<Rc<Self>> {
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

        let (properties, hierarchy_table, contents_table, associated_table) = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = AnsiNodeBTree::read(file, *root.node_btree())?;
            let block_btree = AnsiBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u32::from(node_id))?;
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
            let properties = FolderProperties { properties };

            let node_id = NodeId::new(NodeIdType::HierarchyTable, node_id.index())?;
            let hierarchy_table = if let Ok(node) = node_btree.find_entry(file, u32::from(node_id))
            {
                Some(AnsiTableContext::read(file, encoding, &block_btree, node)?)
            } else {
                None
            };

            let node_id = NodeId::new(NodeIdType::ContentsTable, node_id.index())?;
            let contents_table = if let Ok(node) = node_btree.find_entry(file, u32::from(node_id)) {
                Some(AnsiTableContext::read(file, encoding, &block_btree, node)?)
            } else {
                None
            };

            let node_id = NodeId::new(NodeIdType::AssociatedContentsTable, node_id.index())?;
            let associated_table = if let Ok(node) = node_btree.find_entry(file, u32::from(node_id))
            {
                Some(AnsiTableContext::read(file, encoding, &block_btree, node)?)
            } else {
                None
            };

            (
                properties,
                hierarchy_table,
                contents_table,
                associated_table,
            )
        };

        Ok(Rc::new(Self {
            store,
            properties,
            hierarchy_table,
            contents_table,
            associated_table,
        }))
    }

    pub fn properties(&self) -> &FolderProperties {
        &self.properties
    }

    pub fn hierarchy_table(&self) -> Option<&AnsiTableContext> {
        self.hierarchy_table.as_ref()
    }

    pub fn contents_table(&self) -> Option<&AnsiTableContext> {
        self.contents_table.as_ref()
    }

    pub fn associated_table(&self) -> Option<&AnsiTableContext> {
        self.associated_table.as_ref()
    }
}
