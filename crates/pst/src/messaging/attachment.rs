//! ## [Attachment Objects](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/46eb4828-c6a5-420d-a137-9ee36df317c1)

use std::{collections::BTreeMap, io, rc::Rc};

use super::{message::*, read_write::*, *};
use crate::{
    ltp::{
        heap::HeapNode,
        prop_context::{BinaryValue, PropertyContext, PropertyValue},
        prop_type::PropertyType,
        read_write::*,
    },
    ndb::{
        block::{DataTree, IntermediateTreeBlock, SubNodeTree},
        block_id::BlockId,
        header::Header,
        node_id::{NodeId, NodeIdType},
        page::{BTreePage, NodeBTreeEntry, RootBTree},
        read_write::*,
        root::Root,
    },
    AnsiPstFile, PstFile, PstFileLock, UnicodePstFile,
};

#[derive(Default, Debug)]
pub struct AttachmentProperties {
    properties: BTreeMap<u16, PropertyValue>,
}

impl AttachmentProperties {
    pub fn get(&self, id: u16) -> Option<&PropertyValue> {
        self.properties.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u16, &PropertyValue)> {
        self.properties.iter()
    }

    pub fn attachment_size(&self) -> io::Result<i32> {
        let attachment_size = self
            .properties
            .get(&0x0E20)
            .ok_or(MessagingError::AttachmentSizeNotFound)?;

        match attachment_size {
            PropertyValue::Integer32(value) => Ok(*value),
            invalid => {
                Err(MessagingError::InvalidAttachmentSize(PropertyType::from(invalid)).into())
            }
        }
    }

    pub fn attachment_method(&self) -> io::Result<i32> {
        let attachment_method = self
            .properties
            .get(&0x3705)
            .ok_or(MessagingError::AttachmentMethodNotFound)?;

        match attachment_method {
            PropertyValue::Integer32(value) => Ok(*value),
            invalid => {
                Err(MessagingError::InvalidAttachmentMethod(PropertyType::from(invalid)).into())
            }
        }
    }

    pub fn rendering_position(&self) -> io::Result<i32> {
        let rendering_position = self
            .properties
            .get(&0x370B)
            .ok_or(MessagingError::AttachmentRenderingPositionNotFound)?;

        match rendering_position {
            PropertyValue::Integer32(value) => Ok(*value),
            invalid => Err(
                MessagingError::InvalidAttachmentRenderingPosition(PropertyType::from(invalid))
                    .into(),
            ),
        }
    }
}

/// [PidTagAttachMethod](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcmsg/252923d6-dd41-468b-9c57-d3f68051a516)
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum AttachmentMethod {
    /// `afNone`: The attachment has just been created.
    #[default]
    None = 0x00000000,
    /// `afByValue`: The `PidTagAttachDataBinary` property (section [2.2.2.7](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcmsg/42dfb62b-2ff5-4ffc-ae25-bfdd2db3d8e0))
    /// contains the attachment data.
    ByValue = 0x00000001,
    /// `afByReference`: The `PidTagAttachLongPathname` property (section [2.2.2.13](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcmsg/74b1b39e-1cb4-48ad-b28e-405a261e556c))
    /// contains a fully qualified path identifying the attachment To recipients with access to a
    /// common file server.
    ByReference = 0x00000002,
    /// `afByReferenceOnly`: The `PidTagAttachLongPathname` property contains a fully qualified
    /// path identifying the attachment.
    ByReferenceOnly = 0x00000004,
    /// `afEmbeddedMessage`: The attachment is an embedded message that is accessed via the `RopOpenEmbeddedMessage` ROP ([MS-OXCROPS](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcrops/13af6911-27e5-4aa0-bb75-637b02d4f2ef)
    /// section [2.2.6.16](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcrops/bce79473-e082-4452-822c-ab8cb055dee6)).
    EmbeddedMessage = 0x00000005,
    /// `afStorage`: The `PidTagAttachDataObject` property (section [2.2.2.8](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcmsg/0691206f-0082-463a-a12f-58cb7cb7875f))
    /// contains data in an application-specific format.
    Storage = 0x00000006,
    /// `afByWebReference`: The `PidTagAttachLongPathname` property contains a fully qualified path
    /// identifying the attachment. The `PidNameAttachmentProviderType` defines the web service API
    /// manipulating the attachment.
    ByWebReference = 0x00000007,
}

impl TryFrom<i32> for AttachmentMethod {
    type Error = MessagingError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0x00000000 => Ok(Self::None),
            0x00000001 => Ok(Self::ByValue),
            0x00000002 => Ok(Self::ByReference),
            0x00000004 => Ok(Self::ByReferenceOnly),
            0x00000005 => Ok(Self::EmbeddedMessage),
            0x00000006 => Ok(Self::Storage),
            0x00000007 => Ok(Self::ByWebReference),
            _ => Err(MessagingError::UnknownAttachmentMethod(value)),
        }
    }
}

pub enum AttachmentData {
    Binary(BinaryValue),
    Message(Rc<dyn Message>),
}

pub trait Attachment {
    fn message(&self) -> Rc<dyn Message>;
    fn properties(&self) -> &AttachmentProperties;
    fn data(&self) -> Option<&AttachmentData>;
}

struct AttachmentInner<Pst>
where
    Pst: PstFile,
{
    message: Rc<Pst::Message>,
    properties: AttachmentProperties,
    data: Option<AttachmentData>,
}

impl<Pst> AttachmentInner<Pst>
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
    <Pst as PstFile>::DataTreeBlock: IntermediateTreeBlockReadWrite,
    <<Pst as PstFile>::DataTreeBlock as IntermediateTreeBlock>::Entry:
        IntermediateTreeEntryReadWrite,
    <Pst as PstFile>::DataBlock: BlockReadWrite + Clone,
    <Pst as PstFile>::HeapNode: HeapNodeReadWrite<Pst>,
    <Pst as PstFile>::PropertyTree: HeapTreeReadWrite<Pst>,
    <Pst as PstFile>::PropertyContext: PropertyContextReadWrite<Pst>,
    <Pst as PstFile>::Store: StoreReadWrite<Pst>,
    <Pst as PstFile>::Message: MessageReadWrite<Pst> + 'static,
{
    fn read(
        message: Rc<<Pst as PstFile>::Message>,
        sub_node: NodeId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Self> {
        let node_id_type = sub_node.id_type()?;
        match node_id_type {
            NodeIdType::Attachment => {}
            _ => {
                return Err(MessagingError::InvalidAttachmentNodeIdType(node_id_type).into());
            }
        }

        let store = message.pst_store();
        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let mut embedded = None;
        let (properties, data) = {
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

            let node = message
                .sub_nodes()
                .get(&sub_node)
                .ok_or(MessagingError::AttachmentSubNodeNotFound(sub_node))?;
            let node = <<Pst as PstFile>::NodeBTreeEntry as NodeBTreeEntryReadWrite>::new(
                node.node(),
                node.block(),
                node.sub_node(),
                None,
            );
            let attachment_sub_nodes = node.sub_node();

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
            let prop_context = <<Pst as PstFile>::PropertyContext as PropertyContextReadWrite<
                Pst,
            >>::new(node, tree);
            let properties = prop_context
                .properties()?
                .into_iter()
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, &mut page_cache, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()?;
            let properties = AttachmentProperties { properties };

            let attachment_method = AttachmentMethod::try_from(properties.attachment_method()?)?;
            let data = match attachment_method {
                AttachmentMethod::ByValue => {
                    let binary_data = match properties
                        .get(0x3701)
                        .ok_or(MessagingError::AttachmentMessageObjectDataNotFound)?
                    {
                        PropertyValue::Binary(value) => value,
                        invalid => {
                            return Err(MessagingError::InvalidMessageObjectData(
                                PropertyType::from(invalid),
                            )
                            .into())
                        }
                    };
                    Some(AttachmentData::Binary(binary_data.clone()))
                }
                AttachmentMethod::EmbeddedMessage => {
                    let object_data = match properties
                        .get(0x3701)
                        .ok_or(MessagingError::AttachmentMessageObjectDataNotFound)?
                    {
                        PropertyValue::Object(value) => value,
                        invalid => {
                            return Err(MessagingError::InvalidMessageObjectData(
                                PropertyType::from(invalid),
                            )
                            .into())
                        }
                    };

                    // The embedded message is in the attachment's sub-node tree, not the message's.
                    let sub_node = object_data.node();
                    let attachment_sub_nodes = attachment_sub_nodes
                        .ok_or(MessagingError::AttachmentSubNodeNotFound(sub_node))?;
                    let block = block_btree.find_entry(
                        file,
                        attachment_sub_nodes.search_key(),
                        &mut page_cache,
                    )?;
                    let entry = SubNodeTree::<Pst>::read(file, &block)?
                        .entries(file, &block_btree, &mut page_cache)?
                        .find(|entry| entry.node() == sub_node)
                        .ok_or(MessagingError::AttachmentSubNodeNotFound(sub_node))?;

                    // Read it once the file lock is released, since read_embedded takes it again.
                    embedded = Some(
                        <<Pst as PstFile>::NodeBTreeEntry as NodeBTreeEntryReadWrite>::new(
                            entry.node(),
                            entry.block(),
                            entry.sub_node(),
                            None,
                        ),
                    );
                    None
                }
                AttachmentMethod::Storage => {
                    let object_data = match properties
                        .get(0x3701)
                        .ok_or(MessagingError::AttachmentMessageObjectDataNotFound)?
                    {
                        PropertyValue::Object(value) => value,
                        invalid => {
                            return Err(MessagingError::InvalidMessageObjectData(
                                PropertyType::from(invalid),
                            )
                            .into())
                        }
                    };
                    let sub_node = object_data.node();
                    let node = message
                        .sub_nodes()
                        .get(&sub_node)
                        .ok_or(MessagingError::AttachmentSubNodeNotFound(sub_node))?;
                    let block =
                        block_btree.find_entry(file, node.block().search_key(), &mut page_cache)?;
                    let block = DataTree::read(file, encoding, &block)?;
                    let mut data = vec![];
                    let _ = block
                        .reader(
                            file,
                            encoding,
                            &block_btree,
                            &mut page_cache,
                            &mut Default::default(),
                        )?
                        .read_to_end(&mut data)?;
                    Some(AttachmentData::Binary(BinaryValue::new(data)))
                }
                _ => None,
            };

            (properties, data)
        };

        let data = match embedded {
            Some(node) => Some(AttachmentData::Message(
                <<Pst as PstFile>::Message as MessageReadWrite<Pst>>::read_embedded(
                    store.clone(),
                    node,
                    prop_ids,
                )?,
            )),
            None => data,
        };

        Ok(Self {
            message,
            properties,
            data,
        })
    }
}

pub struct UnicodeAttachment {
    inner: AttachmentInner<UnicodePstFile>,
}

impl UnicodeAttachment {
    pub fn read(
        message: Rc<UnicodeMessage>,
        sub_node: NodeId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        <Self as AttachmentReadWrite<UnicodePstFile>>::read(message, sub_node, prop_ids)
    }
}

impl Attachment for UnicodeAttachment {
    fn message(&self) -> Rc<dyn Message> {
        self.inner.message.clone()
    }

    fn properties(&self) -> &AttachmentProperties {
        &self.inner.properties
    }

    fn data(&self) -> Option<&AttachmentData> {
        self.inner.data.as_ref()
    }
}

impl AttachmentReadWrite<UnicodePstFile> for UnicodeAttachment {
    fn read(
        message: Rc<UnicodeMessage>,
        sub_node: NodeId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        let inner = AttachmentInner::read(message, sub_node, prop_ids)?;
        Ok(Rc::new(Self { inner }))
    }
}

pub struct AnsiAttachment {
    inner: AttachmentInner<AnsiPstFile>,
}

impl AnsiAttachment {
    pub fn read(
        message: Rc<AnsiMessage>,
        sub_node: NodeId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        <Self as AttachmentReadWrite<AnsiPstFile>>::read(message, sub_node, prop_ids)
    }
}

impl Attachment for AnsiAttachment {
    fn message(&self) -> Rc<dyn Message> {
        self.inner.message.clone()
    }

    fn properties(&self) -> &AttachmentProperties {
        &self.inner.properties
    }

    fn data(&self) -> Option<&AttachmentData> {
        self.inner.data.as_ref()
    }
}

impl AttachmentReadWrite<AnsiPstFile> for AnsiAttachment {
    fn read(
        message: Rc<AnsiMessage>,
        sub_node: NodeId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>> {
        let inner = AttachmentInner::read(message, sub_node, prop_ids)?;
        Ok(Rc::new(Self { inner }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ltp::table_context::TableContext,
        messaging::store::{Store, UnicodeStore},
    };

    fn row_ids(table: Option<&Rc<dyn TableContext>>) -> Vec<NodeId> {
        table
            .unwrap()
            .rows_matrix()
            .map(|row| NodeId::from(u32::from(row.id())))
            .collect()
    }

    #[test]
    fn test_read_embedded_message() {
        // pstsdk's submessage.pst has a folder "submessage" with one message, whose only
        // attachment is an embedded message.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/submessage.pst");
        let store = UnicodeStore::read(Rc::new(UnicodePstFile::open(path).unwrap())).unwrap();
        let properties = store.properties();
        let open_folder = |node_id| {
            store
                .open_folder(&properties.make_entry_id(node_id).unwrap())
                .unwrap()
        };
        let ipm_sub_tree = store
            .open_folder(&properties.ipm_sub_tree_entry_id().unwrap())
            .unwrap();
        let folder = row_ids(ipm_sub_tree.hierarchy_table())
            .into_iter()
            .map(open_folder)
            .find(|folder| folder.properties().display_name().unwrap() == "submessage")
            .unwrap();
        let message_id = row_ids(folder.contents_table())[0];
        let entry_id = properties.make_entry_id(message_id).unwrap();
        let message = UnicodeMessage::read(store.clone(), &entry_id, None).unwrap();
        let attachment_id = row_ids(message.attachment_table())[0];

        let attachment = UnicodeAttachment::read(message, attachment_id, None).unwrap();
        let Some(AttachmentData::Message(embedded)) = attachment.data() else {
            panic!("the attachment should be an embedded message");
        };
        let Some(PropertyValue::Unicode(subject)) = embedded.properties().get(0x0037) else {
            panic!("the embedded message should have a subject");
        };
        assert!(subject.to_string().ends_with("This is an embedded message"));
    }
}
