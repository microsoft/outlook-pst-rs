//! ## [Messages](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/dee5b9d0-5513-4c5e-94aa-8bd28a9350b2)

use std::{collections::BTreeMap, io};

use super::{store::*, *};
use crate::{
    ltp::{
        heap::{AnsiHeapNode, UnicodeHeapNode},
        prop_context::{AnsiPropertyContext, PropertyValue, UnicodePropertyContext},
        prop_type::PropertyType,
        tree::{AnsiHeapTree, UnicodeHeapTree},
    },
    ndb::{
        block::{AnsiDataTree, AnsiSubNodeTree, UnicodeDataTree, UnicodeSubNodeTree},
        block_id::{AnsiBlockId, UnicodeBlockId},
        header::Header,
        node_id::{NodeId, NodeIdType},
        page::{
            AnsiBlockBTree, AnsiNodeBTree, NodeBTreeEntry, RootBTree, UnicodeBlockBTree,
            UnicodeNodeBTree,
        },
        root::Root,
    },
};

#[derive(Default, Debug)]
pub struct MessageProperties {
    properties: BTreeMap<u16, PropertyValue>,
}

impl MessageProperties {
    pub fn get(&self, id: u16) -> Option<&PropertyValue> {
        self.properties.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u16, &PropertyValue)> {
        self.properties.iter()
    }

    pub fn message_class(&self) -> io::Result<String> {
        let message_class = self
            .properties
            .get(&0x001A)
            .ok_or(MessagingError::MessageClassNotFound)?;

        match message_class {
            PropertyValue::String8(value) => Ok(value.to_string()),
            PropertyValue::Unicode(value) => Ok(value.to_string()),
            invalid => Err(MessagingError::InvalidMessageClass(PropertyType::from(invalid)).into()),
        }
    }

    pub fn message_flags(&self) -> io::Result<i32> {
        let message_flags = self
            .properties
            .get(&0x0E07)
            .ok_or(MessagingError::MessageFlagsNotFound)?;

        match message_flags {
            PropertyValue::Integer32(value) => Ok(*value),
            invalid => Err(MessagingError::InvalidMessageFlags(PropertyType::from(invalid)).into()),
        }
    }

    pub fn message_size(&self) -> io::Result<i32> {
        let message_size = self
            .properties
            .get(&0x0E08)
            .ok_or(MessagingError::MessageSizeNotFound)?;

        match message_size {
            PropertyValue::Integer32(value) => Ok(*value),
            invalid => Err(MessagingError::InvalidMessageSize(PropertyType::from(invalid)).into()),
        }
    }

    pub fn message_status(&self) -> io::Result<i32> {
        let message_status = self
            .properties
            .get(&0x0E17)
            .ok_or(MessagingError::MessageStatusNotFound)?;

        match message_status {
            PropertyValue::Integer32(value) => Ok(*value),
            invalid => {
                Err(MessagingError::InvalidMessageStatus(PropertyType::from(invalid)).into())
            }
        }
    }

    pub fn creation_time(&self) -> io::Result<i64> {
        let creation_time = self
            .properties
            .get(&0x3007)
            .ok_or(MessagingError::MessageCreationTimeNotFound)?;

        match creation_time {
            PropertyValue::Time(value) => Ok(*value),
            invalid => {
                Err(MessagingError::InvalidMessageCreationTime(PropertyType::from(invalid)).into())
            }
        }
    }

    pub fn last_modification_time(&self) -> io::Result<i64> {
        let last_modification_time = self
            .properties
            .get(&0x3008)
            .ok_or(MessagingError::MessageLastModificationTimeNotFound)?;

        match last_modification_time {
            PropertyValue::Time(value) => Ok(*value),
            invalid => Err(
                MessagingError::InvalidMessageLastModificationTime(PropertyType::from(invalid))
                    .into(),
            ),
        }
    }

    pub fn search_key(&self) -> io::Result<&[u8]> {
        let search_key = self
            .properties
            .get(&0x300B)
            .ok_or(MessagingError::MessageSearchKeyNotFound)?;

        match search_key {
            PropertyValue::Binary(value) => Ok(value.buffer()),
            invalid => {
                Err(MessagingError::InvalidMessageSearchKey(PropertyType::from(invalid)).into())
            }
        }
    }
}

pub type UnicodeSubNodeMap = BTreeMap<NodeId, UnicodeBlockId>;
pub type AnsiSubNodeMap = BTreeMap<NodeId, AnsiBlockId>;

pub struct UnicodeMessage<'a> {
    store: &'a UnicodeStore<'a>,
    properties: MessageProperties,
    sub_nodes: UnicodeSubNodeMap,
}

impl<'a> UnicodeMessage<'a> {
    pub fn store(&self) -> &UnicodeStore {
        self.store
    }

    pub fn read(store: &'a UnicodeStore, entry_id: &EntryId) -> io::Result<Self> {
        let node_id = entry_id.node_id();
        let node_id_type = node_id.id_type()?;
        match node_id_type {
            NodeIdType::NormalMessage | NodeIdType::AssociatedMessage | NodeIdType::Attachment => {}
            _ => {
                return Err(MessagingError::InvalidMessageEntryIdType(node_id_type).into());
            }
        }
        if !store.matches_record_key(entry_id)? {
            return Err(MessagingError::EntryIdWrongStore.into());
        }

        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let (properties, sub_nodes) = {
            let mut file = pst
                .file()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = UnicodeNodeBTree::read(file, *root.node_btree())?;
            let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u64::from(u32::from(node_id)))?;
            let sub_node = node
                .sub_node()
                .ok_or(MessagingError::MessageSubNodeTreeNotFound)?;

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
            let properties = MessageProperties { properties };

            let block = block_btree.find_entry(file, u64::from(sub_node))?;
            let sub_nodes = UnicodeSubNodeTree::read(file, &block)?;
            let sub_nodes = sub_nodes
                .entries(file, &block_btree)?
                .map(|entry| (entry.node(), entry.block()))
                .collect();

            (properties, sub_nodes)
        };

        Ok(Self {
            store,
            properties,
            sub_nodes,
        })
    }

    pub fn properties(&self) -> &MessageProperties {
        &self.properties
    }

    pub fn sub_nodes(&self) -> &UnicodeSubNodeMap {
        &self.sub_nodes
    }
}

pub struct AnsiMessage<'a> {
    store: &'a AnsiStore<'a>,
    properties: MessageProperties,
    sub_nodes: AnsiSubNodeMap,
}

impl<'a> AnsiMessage<'a> {
    pub fn store(&self) -> &AnsiStore {
        self.store
    }

    pub fn read(store: &'a AnsiStore, entry_id: &EntryId) -> io::Result<Self> {
        let node_id = entry_id.node_id();
        let node_id_type = node_id.id_type()?;
        match node_id_type {
            NodeIdType::NormalMessage | NodeIdType::AssociatedMessage | NodeIdType::Attachment => {}
            _ => {
                return Err(MessagingError::InvalidMessageEntryIdType(node_id_type).into());
            }
        }
        if !store.matches_record_key(entry_id)? {
            return Err(MessagingError::EntryIdWrongStore.into());
        }

        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let (properties, sub_nodes) = {
            let mut file = pst
                .file()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = AnsiNodeBTree::read(file, *root.node_btree())?;
            let block_btree = AnsiBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u32::from(node_id))?;
            let sub_node = node
                .sub_node()
                .ok_or(MessagingError::MessageSubNodeTreeNotFound)?;

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
            let properties = MessageProperties { properties };

            let block = block_btree.find_entry(file, u32::from(sub_node))?;
            let sub_nodes = AnsiSubNodeTree::read(file, &block)?;
            let sub_nodes = sub_nodes
                .entries(file, &block_btree)?
                .map(|entry| (entry.node(), entry.block()))
                .collect();

            (properties, sub_nodes)
        };

        Ok(Self {
            store,
            properties,
            sub_nodes,
        })
    }

    pub fn properties(&self) -> &MessageProperties {
        &self.properties
    }

    pub fn sub_nodes(&self) -> &AnsiSubNodeMap {
        &self.sub_nodes
    }
}
