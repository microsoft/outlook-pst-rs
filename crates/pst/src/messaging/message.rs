//! ## [Message Objects](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/1042af37-aaa4-4edc-bffd-90a1ede24188)

use std::{collections::BTreeMap, io, rc::Rc};

use super::{read_write::*, store::*, *};
use crate::{
    ltp::{
        heap::HeapNode,
        prop_context::{PropertyContext, PropertyValue},
        prop_type::PropertyType,
        read_write::*,
        table_context::TableContext,
    },
    ndb::{
        block::{IntermediateTreeBlock, LeafSubNodeTreeEntry, SubNodeTree},
        block_id::BlockId,
        header::Header,
        node_id::{NodeId, NodeIdType},
        page::{AnsiNodeBTreeEntry, BTreePage, NodeBTreeEntry, RootBTree, UnicodeNodeBTreeEntry},
        read_write::*,
        root::Root,
    },
    AnsiPstFile, PstFile, PstFileLock, UnicodePstFile,
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

pub trait Message {
    fn store(&self) -> Rc<dyn Store>;
    fn properties(&self) -> &MessageProperties;
    fn recipient_table(&self) -> Option<&Rc<dyn TableContext>>;
    fn attachment_table(&self) -> Option<&Rc<dyn TableContext>>;
}

struct MessageInner<Pst>
where
    Pst: PstFile,
{
    store: Rc<Pst::Store>,
    properties: MessageProperties,
    sub_nodes: MessageSubNodes<Pst>,
    recipient_table: Option<Rc<dyn TableContext>>,
    attachment_table: Option<Rc<dyn TableContext>>,
}

impl<Pst> MessageInner<Pst>
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
    <Pst as PstFile>::SubNodeTreeBlockHeader: IntermediateTreeHeaderReadWrite,
    <Pst as PstFile>::SubNodeTreeBlock: IntermediateTreeBlockReadWrite,
    <<Pst as PstFile>::SubNodeTreeBlock as IntermediateTreeBlock>::Entry:
        IntermediateTreeEntryReadWrite,
    <Pst as PstFile>::SubNodeBlock: IntermediateTreeBlockReadWrite,
    <<Pst as PstFile>::SubNodeBlock as IntermediateTreeBlock>::Entry:
        IntermediateTreeEntryReadWrite,
    <Pst as PstFile>::HeapNode: HeapNodeReadWrite<Pst>,
    <Pst as PstFile>::PropertyTree: HeapTreeReadWrite<Pst>,
    <Pst as PstFile>::TableContext: TableContextReadWrite<Pst>,
    <Pst as PstFile>::PropertyContext: PropertyContextReadWrite<Pst>,
    <Pst as PstFile>::Store: StoreReadWrite<Pst>,
{
    fn read(
        store: Rc<<Pst as PstFile>::Store>,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Self> {
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

            let node_btree = <<Pst as PstFile>::NodeBTree as RootBTreeReadWrite>::read(
                file,
                *root.node_btree(),
            )?;

            let mut page_cache = pst.node_cache();
            let node_key: <Pst as PstFile>::BTreeKey = u32::from(node_id).into();
            node_btree.find_entry(file, node_key, &mut page_cache)?
        };

        Self::read_embedded(store, node, prop_ids)
    }

    fn read_embedded(
        store: Rc<<Pst as PstFile>::Store>,
        node: <Pst as PstFile>::NodeBTreeEntry,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Self> {
        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let (properties, sub_nodes) = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let block_btree = <<Pst as PstFile>::BlockBTree as RootBTreeReadWrite>::read(
                file,
                *root.block_btree(),
            )?;

            let sub_node = node
                .sub_node()
                .ok_or(MessagingError::MessageSubNodeTreeNotFound)?;

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
                .filter(|(prop_id, _)| prop_ids.is_none_or(|ids| ids.contains(prop_id)))
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, &mut page_cache, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()?;
            let properties = MessageProperties { properties };

            let block = block_btree.find_entry(file, sub_node.search_key(), &mut page_cache)?;
            let sub_nodes = SubNodeTree::<Pst>::read(file, &block)?;
            let sub_nodes: BTreeMap<_, _> = sub_nodes
                .entries(file, &block_btree, &mut page_cache)?
                .map(|entry| (entry.node(), entry))
                .collect();

            (properties, sub_nodes)
        };

        let mut recipient_table_nodes = sub_nodes.iter().filter_map(|(node_id, entry)| {
            node_id.id_type().ok().and_then(|id_type| {
                if id_type == NodeIdType::RecipientTable {
                    Some(
                        <<Pst as PstFile>::NodeBTreeEntry as NodeBTreeEntryReadWrite>::new(
                            entry.node(),
                            entry.block(),
                            entry.sub_node(),
                            None,
                        ),
                    )
                } else {
                    None
                }
            })
        });
        let recipient_table = match (recipient_table_nodes.next(), recipient_table_nodes.next()) {
            (None, None) => None,
            (Some(node), None) => {
                Some(<<Pst as PstFile>::TableContext as TableContextReadWrite<
                    Pst,
                >>::read(store.clone(), node)?)
            }
            _ => return Err(MessagingError::MultipleMessageRecipientTables.into()),
        };

        let mut attachment_table_nodes = sub_nodes.iter().filter_map(|(node_id, entry)| {
            node_id.id_type().ok().and_then(|id_type| {
                if id_type == NodeIdType::AttachmentTable {
                    Some(
                        <<Pst as PstFile>::NodeBTreeEntry as NodeBTreeEntryReadWrite>::new(
                            entry.node(),
                            entry.block(),
                            entry.sub_node(),
                            None,
                        ),
                    )
                } else {
                    None
                }
            })
        });
        let attachment_table = match (attachment_table_nodes.next(), attachment_table_nodes.next())
        {
            (None, None) => None,
            (Some(node), None) => {
                Some(<<Pst as PstFile>::TableContext as TableContextReadWrite<
                    Pst,
                >>::read(store.clone(), node)?)
            }
            _ => return Err(MessagingError::MultipleMessageAttachmentTables.into()),
        };

        Ok(Self {
            store,
            properties,
            sub_nodes,
            recipient_table,
            attachment_table,
        })
    }
}

pub type MessageSubNodes<Pst> = BTreeMap<NodeId, LeafSubNodeTreeEntry<<Pst as PstFile>::BlockId>>;
pub type UnicodeMessageSubNodes = MessageSubNodes<UnicodePstFile>;
pub type AnsiMessageSubNodes = MessageSubNodes<AnsiPstFile>;

pub struct UnicodeMessage {
    inner: MessageInner<UnicodePstFile>,
}

impl UnicodeMessage {
    pub fn read(
        store: Rc<UnicodeStore>,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        <Self as MessageReadWrite<UnicodePstFile>>::read(store, entry_id, prop_ids)
    }
}

impl Message for UnicodeMessage {
    fn store(&self) -> Rc<dyn Store> {
        self.inner.store.clone()
    }

    fn properties(&self) -> &MessageProperties {
        &self.inner.properties
    }

    fn recipient_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.recipient_table.as_ref()
    }

    fn attachment_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.attachment_table.as_ref()
    }
}

impl MessageReadWrite<UnicodePstFile> for UnicodeMessage {
    fn read(
        store: Rc<UnicodeStore>,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        let inner = MessageInner::read(store, entry_id, prop_ids)?;
        Ok(Rc::new(Self { inner }))
    }

    fn read_embedded(
        store: Rc<UnicodeStore>,
        node: UnicodeNodeBTreeEntry,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        let inner = MessageInner::read_embedded(store, node, prop_ids)?;
        Ok(Rc::new(Self { inner }))
    }

    fn pst_store(&self) -> &Rc<UnicodeStore> {
        &self.inner.store
    }

    fn sub_nodes(&self) -> &UnicodeMessageSubNodes {
        &self.inner.sub_nodes
    }
}

pub struct AnsiMessage {
    inner: MessageInner<AnsiPstFile>,
}

impl AnsiMessage {
    pub fn read(
        store: Rc<AnsiStore>,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        <Self as MessageReadWrite<AnsiPstFile>>::read(store, entry_id, prop_ids)
    }
}

impl Message for AnsiMessage {
    fn store(&self) -> Rc<dyn Store> {
        self.inner.store.clone()
    }

    fn properties(&self) -> &MessageProperties {
        &self.inner.properties
    }

    fn recipient_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.recipient_table.as_ref()
    }

    fn attachment_table(&self) -> Option<&Rc<dyn TableContext>> {
        self.inner.attachment_table.as_ref()
    }
}

impl MessageReadWrite<AnsiPstFile> for AnsiMessage {
    fn read(
        store: Rc<AnsiStore>,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        let inner = MessageInner::read(store, entry_id, prop_ids)?;
        Ok(Rc::new(Self { inner }))
    }

    fn read_embedded(
        store: Rc<AnsiStore>,
        node: AnsiNodeBTreeEntry,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        let inner = MessageInner::read_embedded(store, node, prop_ids)?;
        Ok(Rc::new(Self { inner }))
    }

    fn pst_store(&self) -> &Rc<AnsiStore> {
        &self.inner.store
    }

    fn sub_nodes(&self) -> &AnsiMessageSubNodes {
        &self.inner.sub_nodes
    }
}
