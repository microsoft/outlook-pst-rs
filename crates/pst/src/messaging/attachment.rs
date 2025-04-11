//! ## [Attachment Objects](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/46eb4828-c6a5-420d-a137-9ee36df317c1)

use std::{collections::BTreeMap, io, sync::Arc};

use super::{message::*, *};
use crate::{
    ltp::{
        heap::{AnsiHeapNode, UnicodeHeapNode},
        prop_context::{AnsiPropertyContext, BinaryValue, PropertyValue, UnicodePropertyContext},
        prop_type::PropertyType,
        tree::{AnsiHeapTree, UnicodeHeapTree},
    },
    ndb::{
        block::{AnsiDataTree, UnicodeDataTree},
        header::Header,
        node_id::{NodeId, NodeIdType},
        page::{
            AnsiBlockBTree, AnsiBlockBTreeEntry, AnsiNodeBTreeEntry, NodeBTreeEntry,
            UnicodeBlockBTree, UnicodeBlockBTreeEntry, UnicodeNodeBTreeEntry,
        },
        root::Root,
    },
    PstFile,
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

pub enum UnicodeAttachmentData {
    Binary(BinaryValue),
    Message(Arc<UnicodeMessage>),
    Storage(UnicodeBlockBTreeEntry),
}

pub struct UnicodeAttachment {
    message: Arc<UnicodeMessage>,
    properties: AttachmentProperties,
    data: Option<UnicodeAttachmentData>,
}

impl UnicodeAttachment {
    pub fn message(&self) -> &Arc<UnicodeMessage> {
        &self.message
    }

    pub fn read(
        message: Arc<UnicodeMessage>,
        sub_node: NodeId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Arc<Self>> {
        let node_id_type = sub_node.id_type()?;
        match node_id_type {
            NodeIdType::Attachment => {}
            _ => {
                return Err(MessagingError::InvalidAttachmentNodeIdType(node_id_type).into());
            }
        }

        let store = message.store();
        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let (properties, data) = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;

            let node = message
                .sub_nodes()
                .get(&sub_node)
                .ok_or(MessagingError::AttachmentSubNodeNotFound(sub_node))?;
            let node = UnicodeNodeBTreeEntry::new(node.node(), node.block(), node.sub_node(), None);

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
                    Some(UnicodeAttachmentData::Binary(binary_data.clone()))
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

                    let sub_node = object_data.node();
                    let node = message
                        .sub_nodes()
                        .get(&sub_node)
                        .ok_or(MessagingError::AttachmentSubNodeNotFound(sub_node))?;
                    let node = UnicodeNodeBTreeEntry::new(
                        node.node(),
                        node.block(),
                        node.sub_node(),
                        None,
                    );
                    let message = UnicodeMessage::read_embedded(store.clone(), node, prop_ids)?;
                    Some(UnicodeAttachmentData::Message(message))
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
                    let block = block_btree.find_entry(file, u64::from(node.block()))?;
                    Some(UnicodeAttachmentData::Storage(block))
                }
                _ => None,
            };

            (properties, data)
        };

        Ok(Arc::new(Self {
            message,
            properties,
            data,
        }))
    }

    pub fn properties(&self) -> &AttachmentProperties {
        &self.properties
    }

    pub fn data(&self) -> Option<&UnicodeAttachmentData> {
        self.data.as_ref()
    }
}

pub enum AnsiAttachmentData {
    Binary(BinaryValue),
    Message(Arc<AnsiMessage>),
    Storage(AnsiBlockBTreeEntry),
}

pub struct AnsiAttachment {
    message: Arc<AnsiMessage>,
    properties: AttachmentProperties,
    data: Option<AnsiAttachmentData>,
}

impl AnsiAttachment {
    pub fn message(&self) -> &Arc<AnsiMessage> {
        &self.message
    }

    pub fn read(
        message: Arc<AnsiMessage>,
        sub_node: NodeId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Arc<Self>> {
        let node_id_type = sub_node.id_type()?;
        match node_id_type {
            NodeIdType::Attachment => {}
            _ => {
                return Err(MessagingError::InvalidAttachmentNodeIdType(node_id_type).into());
            }
        }

        let store = message.store();
        let pst = store.pst();
        let header = pst.header();
        let root = header.root();

        let (properties, data) = {
            let mut file = pst
                .reader()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let block_btree = AnsiBlockBTree::read(file, *root.block_btree())?;

            let node = message
                .sub_nodes()
                .get(&sub_node)
                .ok_or(MessagingError::AttachmentSubNodeNotFound(sub_node))?;
            let node = AnsiNodeBTreeEntry::new(node.node(), node.block(), node.sub_node(), None);

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
                    Some(AnsiAttachmentData::Binary(binary_data.clone()))
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

                    let sub_node = object_data.node();
                    let node = message
                        .sub_nodes()
                        .get(&sub_node)
                        .ok_or(MessagingError::AttachmentSubNodeNotFound(sub_node))?;
                    let node =
                        AnsiNodeBTreeEntry::new(node.node(), node.block(), node.sub_node(), None);
                    let message = AnsiMessage::read_embedded(store.clone(), node, prop_ids)?;
                    Some(AnsiAttachmentData::Message(message))
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
                    let block = block_btree.find_entry(file, u32::from(node.block()))?;
                    Some(AnsiAttachmentData::Storage(block))
                }
                _ => None,
            };

            (properties, data)
        };

        Ok(Arc::new(Self {
            message,
            properties,
            data,
        }))
    }

    pub fn properties(&self) -> &AttachmentProperties {
        &self.properties
    }

    pub fn data(&self) -> Option<&AnsiAttachmentData> {
        self.data.as_ref()
    }
}
