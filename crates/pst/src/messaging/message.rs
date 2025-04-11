//! ## [Message Objects](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/1042af37-aaa4-4edc-bffd-90a1ede24188)

use std::{collections::BTreeMap, io, sync::Arc};

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
        block::{
            AnsiDataTree, AnsiLeafSubNodeTreeEntry, AnsiSubNodeTree, UnicodeDataTree,
            UnicodeLeafSubNodeTreeEntry, UnicodeSubNodeTree,
        },
        header::Header,
        node_id::{NodeId, NodeIdType},
        page::{
            AnsiBlockBTree, AnsiNodeBTree, AnsiNodeBTreeEntry, NodeBTreeEntry, UnicodeBlockBTree,
            UnicodeNodeBTree, UnicodeNodeBTreeEntry,
        },
        root::Root,
    },
    PstFile,
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

pub type UnicodeMessageSubNodes = BTreeMap<NodeId, UnicodeLeafSubNodeTreeEntry>;
pub type AnsiMessageSubNodes = BTreeMap<NodeId, AnsiLeafSubNodeTreeEntry>;

pub struct UnicodeMessage {
    store: Arc<UnicodeStore>,
    properties: MessageProperties,
    sub_nodes: UnicodeMessageSubNodes,
    recipient_table: UnicodeTableContext,
    attachment_table: Option<UnicodeTableContext>,
}

impl UnicodeMessage {
    pub fn store(&self) -> &Arc<UnicodeStore> {
        &self.store
    }

    pub fn read(
        store: Arc<UnicodeStore>,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Arc<Self>> {
        let node_id = entry_id.node_id();
        let node_id_type = node_id.id_type()?;
        match node_id_type {
            NodeIdType::NormalMessage | NodeIdType::AssociatedMessage | NodeIdType::Attachment => {}
            _ => {
                return Err(MessagingError::InvalidMessageEntryIdType(node_id_type).into());
            }
        }
        if !store.properties().matches_record_key(entry_id)? {
            return Err(MessagingError::EntryIdWrongStore.into());
        }

        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let node = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let node_btree = UnicodeNodeBTree::read(file, *root.node_btree())?;

            node_btree.find_entry(file, u64::from(u32::from(node_id)))?
        };

        Self::read_embedded(store, node, prop_ids)
    }

    pub fn read_embedded(
        store: Arc<UnicodeStore>,
        node: UnicodeNodeBTreeEntry,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Arc<Self>> {
        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let (properties, sub_nodes, recipient_table, attachment_table) = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;

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
                .filter(|(prop_id, _)| prop_ids.map_or(true, |ids| ids.contains(prop_id)))
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()?;
            let properties = MessageProperties { properties };

            let block = block_btree.find_entry(file, u64::from(sub_node))?;
            let sub_nodes = UnicodeSubNodeTree::read(file, &block)?;
            let sub_nodes: UnicodeMessageSubNodes = sub_nodes
                .entries(file, &block_btree)?
                .map(|entry| (entry.node(), entry))
                .collect();

            let mut recipient_table_nodes = sub_nodes.iter().filter_map(|(node_id, entry)| {
                node_id.id_type().ok().and_then(|id_type| {
                    if id_type == NodeIdType::RecipientTable {
                        Some(UnicodeNodeBTreeEntry::new(
                            entry.node(),
                            entry.block(),
                            entry.sub_node(),
                            None,
                        ))
                    } else {
                        None
                    }
                })
            });
            let recipient_table = match (recipient_table_nodes.next(), recipient_table_nodes.next())
            {
                (None, None) => Err(MessagingError::MessageRecipientTableNotFound.into()),
                (Some(node), None) => UnicodeTableContext::read(file, encoding, &block_btree, node),
                _ => Err(MessagingError::MultipleMessageRecipientTables.into()),
            }?;

            let mut attachment_table_nodes = sub_nodes.iter().filter_map(|(node_id, entry)| {
                node_id.id_type().ok().and_then(|id_type| {
                    if id_type == NodeIdType::AttachmentTable {
                        Some(UnicodeNodeBTreeEntry::new(
                            entry.node(),
                            entry.block(),
                            entry.sub_node(),
                            None,
                        ))
                    } else {
                        None
                    }
                })
            });
            let attachment_table =
                match (attachment_table_nodes.next(), attachment_table_nodes.next()) {
                    (None, None) => None,
                    (Some(node), None) => Some(UnicodeTableContext::read(
                        file,
                        encoding,
                        &block_btree,
                        node,
                    )?),
                    _ => return Err(MessagingError::MultipleMessageAttachmentTables.into()),
                };

            (properties, sub_nodes, recipient_table, attachment_table)
        };

        Ok(Arc::new(Self {
            store,
            properties,
            sub_nodes,
            recipient_table,
            attachment_table,
        }))
    }

    pub fn properties(&self) -> &MessageProperties {
        &self.properties
    }

    pub fn sub_nodes(&self) -> &UnicodeMessageSubNodes {
        &self.sub_nodes
    }

    pub fn recipient_table(&self) -> &UnicodeTableContext {
        &self.recipient_table
    }

    pub fn attachment_table(&self) -> Option<&UnicodeTableContext> {
        self.attachment_table.as_ref()
    }
}

pub struct AnsiMessage {
    store: Arc<AnsiStore>,
    properties: MessageProperties,
    sub_nodes: AnsiMessageSubNodes,
    recipient_table: AnsiTableContext,
    attachment_table: Option<AnsiTableContext>,
}

impl AnsiMessage {
    pub fn store(&self) -> &Arc<AnsiStore> {
        &self.store
    }

    pub fn read(
        store: Arc<AnsiStore>,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Arc<Self>> {
        let node_id = entry_id.node_id();
        let node_id_type = node_id.id_type()?;
        match node_id_type {
            NodeIdType::NormalMessage | NodeIdType::AssociatedMessage | NodeIdType::Attachment => {}
            _ => {
                return Err(MessagingError::InvalidMessageEntryIdType(node_id_type).into());
            }
        }
        if !store.properties().matches_record_key(entry_id)? {
            return Err(MessagingError::EntryIdWrongStore.into());
        }

        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let node = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let node_btree = AnsiNodeBTree::read(file, *root.node_btree())?;

            node_btree.find_entry(file, u32::from(node_id))?
        };

        Self::read_embedded(store, node, prop_ids)
    }

    pub fn read_embedded(
        store: Arc<AnsiStore>,
        node: AnsiNodeBTreeEntry,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Arc<Self>> {
        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let (properties, sub_nodes, recipient_table, attachment_table) = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let block_btree = AnsiBlockBTree::read(file, *root.block_btree())?;

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
                .filter(|(prop_id, _)| prop_ids.map_or(true, |ids| ids.contains(prop_id)))
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()?;
            let properties = MessageProperties { properties };

            let block = block_btree.find_entry(file, u32::from(sub_node))?;
            let sub_nodes = AnsiSubNodeTree::read(file, &block)?;
            let sub_nodes: AnsiMessageSubNodes = sub_nodes
                .entries(file, &block_btree)?
                .map(|entry| (entry.node(), entry))
                .collect();

            let mut recipient_table_nodes = sub_nodes.iter().filter_map(|(node_id, entry)| {
                node_id.id_type().ok().and_then(|id_type| {
                    if id_type == NodeIdType::RecipientTable {
                        Some(AnsiNodeBTreeEntry::new(
                            entry.node(),
                            entry.block(),
                            entry.sub_node(),
                            None,
                        ))
                    } else {
                        None
                    }
                })
            });
            let recipient_table = match (recipient_table_nodes.next(), recipient_table_nodes.next())
            {
                (None, None) => Err(MessagingError::MessageRecipientTableNotFound.into()),
                (Some(node), None) => AnsiTableContext::read(file, encoding, &block_btree, node),
                _ => Err(MessagingError::MultipleMessageRecipientTables.into()),
            }?;

            let mut attachment_table_nodes = sub_nodes.iter().filter_map(|(node_id, entry)| {
                node_id.id_type().ok().and_then(|id_type| {
                    if id_type == NodeIdType::AttachmentTable {
                        Some(AnsiNodeBTreeEntry::new(
                            entry.node(),
                            entry.block(),
                            entry.sub_node(),
                            None,
                        ))
                    } else {
                        None
                    }
                })
            });
            let attachment_table =
                match (attachment_table_nodes.next(), attachment_table_nodes.next()) {
                    (None, None) => None,
                    (Some(node), None) => {
                        Some(AnsiTableContext::read(file, encoding, &block_btree, node)?)
                    }
                    _ => return Err(MessagingError::MultipleMessageAttachmentTables.into()),
                };

            (properties, sub_nodes, recipient_table, attachment_table)
        };

        Ok(Arc::new(Self {
            store,
            properties,
            sub_nodes,
            recipient_table,
            attachment_table,
        }))
    }

    pub fn properties(&self) -> &MessageProperties {
        &self.properties
    }

    pub fn sub_nodes(&self) -> &AnsiMessageSubNodes {
        &self.sub_nodes
    }

    pub fn recipient_table(&self) -> &AnsiTableContext {
        &self.recipient_table
    }

    pub fn attachment_table(&self) -> Option<&AnsiTableContext> {
        self.attachment_table.as_ref()
    }
}
