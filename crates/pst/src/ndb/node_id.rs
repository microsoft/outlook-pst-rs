//! [NID (Node ID)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/18d7644e-cb33-4e11-95c0-34d8a84fbff6)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::{
    fmt::Debug,
    io::{self, Read, Write},
};

use super::{read_write::NodeIdReadWrite, *};

/// `nidType`
///
/// ### See also
/// [NodeId]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum NodeIdType {
    /// `NID_TYPE_HID`: Heap node
    HeapNode = 0x00,
    /// `NID_TYPE_INTERNAL`: Internal node
    Internal = 0x01,
    /// `NID_TYPE_NORMAL_FOLDER`: Normal Folder object (PC)
    NormalFolder = 0x02,
    /// `NID_TYPE_SEARCH_FOLDER`: Search Folder object (PC)
    SearchFolder = 0x03,
    /// `NID_TYPE_NORMAL_MESSAGE`: Normal Message object (PC)
    NormalMessage = 0x04,
    /// `NID_TYPE_ATTACHMENT`: Attachment object (PC)
    Attachment = 0x05,
    /// `NID_TYPE_SEARCH_UPDATE_QUEUE`: Queue of changed objects for search Folder objects
    SearchUpdateQueue = 0x06,
    /// `NID_TYPE_SEARCH_CRITERIA_OBJECT`: Defines the search criteria for a search Folder object
    SearchCriteria = 0x07,
    /// `NID_TYPE_ASSOC_MESSAGE`: Folder associated information (FAI) Message object (PC)
    AssociatedMessage = 0x08,
    /// `NID_TYPE_CONTENTS_TABLE_INDEX`: Internal, persisted view-related
    ContentsTableIndex = 0x0A,
    /// `NID_TYPE_RECEIVE_FOLDER_TABLE`: Receive Folder object (Inbox)
    ReceiveFolderTable = 0x0B,
    /// `NID_TYPE_OUTGOING_QUEUE_TABLE`: Outbound queue (Outbox)
    OutgoingQueueTable = 0x0C,
    /// `NID_TYPE_HIERARCHY_TABLE`: Hierarchy table (TC)
    HierarchyTable = 0x0D,
    /// `NID_TYPE_CONTENTS_TABLE`: Contents table (TC)
    ContentsTable = 0x0E,
    /// `NID_TYPE_ASSOC_CONTENTS_TABLE`: FAI contents table (TC)
    AssociatedContentsTable = 0x0F,
    /// `NID_TYPE_SEARCH_CONTENTS_TABLE`: Contents table (TC) of a search Folder object
    SearchContentsTable = 0x10,
    /// `NID_TYPE_ATTACHMENT_TABLE`: Attachment table (TC)
    AttachmentTable = 0x11,
    /// `NID_TYPE_RECIPIENT_TABLE`: Recipient table (TC)
    RecipientTable = 0x12,
    /// `NID_TYPE_SEARCH_TABLE_INDEX`: Internal, persisted view-related
    SearchTableIndex = 0x13,
    /// `NID_TYPE_LTP`: [LTP](crate::ltp)
    ListsTablesProperties = 0x1F,
}

impl TryFrom<u8> for NodeIdType {
    type Error = NdbError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(NodeIdType::HeapNode),
            0x01 => Ok(NodeIdType::Internal),
            0x02 => Ok(NodeIdType::NormalFolder),
            0x03 => Ok(NodeIdType::SearchFolder),
            0x04 => Ok(NodeIdType::NormalMessage),
            0x05 => Ok(NodeIdType::Attachment),
            0x06 => Ok(NodeIdType::SearchUpdateQueue),
            0x07 => Ok(NodeIdType::SearchCriteria),
            0x08 => Ok(NodeIdType::AssociatedMessage),
            0x0A => Ok(NodeIdType::ContentsTableIndex),
            0x0B => Ok(NodeIdType::ReceiveFolderTable),
            0x0C => Ok(NodeIdType::OutgoingQueueTable),
            0x0D => Ok(NodeIdType::HierarchyTable),
            0x0E => Ok(NodeIdType::ContentsTable),
            0x0F => Ok(NodeIdType::AssociatedContentsTable),
            0x10 => Ok(NodeIdType::SearchContentsTable),
            0x11 => Ok(NodeIdType::AttachmentTable),
            0x12 => Ok(NodeIdType::RecipientTable),
            0x13 => Ok(NodeIdType::SearchTableIndex),
            0x1F => Ok(NodeIdType::ListsTablesProperties),
            _ => Err(NdbError::InvalidNodeIdType(value)),
        }
    }
}

pub const MAX_NODE_INDEX: u32 = 1_u32.rotate_right(5) - 1;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    pub fn new(id_type: NodeIdType, index: u32) -> NdbResult<Self> {
        let id_type = id_type as u8;
        if id_type >> 5 != 0 {
            return Err(NdbError::InvalidNodeIdType(id_type));
        }

        let shifted_index = index.rotate_left(5);
        if shifted_index & 0x1F != 0 {
            return Err(NdbError::InvalidNodeIndex(index));
        };

        Ok(Self(shifted_index | (u32::from(id_type))))
    }

    pub fn id_type(&self) -> NdbResult<NodeIdType> {
        let nid_type = self.0 & 0x1F;
        NodeIdType::try_from(nid_type as u8)
    }

    pub fn index(&self) -> u32 {
        self.0 >> 5
    }
}

impl NodeIdReadWrite for NodeId {
    fn new(id_type: NodeIdType, index: u32) -> NdbResult<Self> {
        Self::new(id_type, index)
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u32::<LittleEndian>()?;
        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.0)
    }
}

impl Debug for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Ok(id_type) = self.id_type() else {
            return write!(f, "NodeId {{ invalid: 0x{:08X} }}", u32::from(*self));
        };

        write!(f, "NodeId {{ {:?}: 0x{:X} }}", id_type, self.index())
    }
}

impl From<u32> for NodeId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<NodeId> for u32 {
    fn from(value: NodeId) -> Self {
        value.0
    }
}

/// [`NID_MESSAGE_STORE`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Message store node (section [2.4.3](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/aa0539bd-e7bf-4cec-8bde-0b87c2a86baf)).
pub const NID_MESSAGE_STORE: NodeId = NodeId(0x21);

/// [`NID_NAME_TO_ID_MAP`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Named Properties Map (section [2.4.7](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/e17e195d-0454-4b9b-b398-c9127a26a678)).
pub const NID_NAME_TO_ID_MAP: NodeId = NodeId(0x61);

/// [`NID_NORMAL_FOLDER_TEMPLATE`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Special template node for an empty Folder object.
pub const NID_NORMAL_FOLDER_TEMPLATE: NodeId = NodeId(0xA1);

/// [`NID_SEARCH_FOLDER_TEMPLATE`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Special template node for an empty search Folder object.
pub const NID_SEARCH_FOLDER_TEMPLATE: NodeId = NodeId(0xC1);

/// [`NID_ROOT_FOLDER`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Root Mailbox Folder object of PST.
pub const NID_ROOT_FOLDER: NodeId = NodeId(0x122);

/// [`NID_SEARCH_MANAGEMENT_QUEUE`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Queue of Pending Search-related updates.
pub const NID_SEARCH_MANAGEMENT_QUEUE: NodeId = NodeId(0x1E1);

/// [`NID_SEARCH_ACTIVITY_LIST`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Folder object NIDs with active Search activity.
pub const NID_SEARCH_ACTIVITY_LIST: NodeId = NodeId(0x201);

/// [`NID_RESERVED1`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Reserved.
pub const NID_RESERVED1: NodeId = NodeId(0x241);

/// [`NID_SEARCH_DOMAIN_OBJECT`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Global list of all Folder objects that are referenced by any Folder object's Search Criteria.
pub const NID_SEARCH_DOMAIN_OBJECT: NodeId = NodeId(0x261);

/// [`NID_SEARCH_GATHERER_QUEUE`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Search Gatherer Queue (section [2.4.8.5.1](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/984f26d3-6603-4229-974f-4373e5a95c6a)).
pub const NID_SEARCH_GATHERER_QUEUE: NodeId = NodeId(0x281);

/// [`NID_SEARCH_GATHERER_DESCRIPTOR`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Search Gatherer Descriptor (section [2.4.8.5.2](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/45ac5a7b-6698-4dbd-8a34-e499b79199b9)).
pub const NID_SEARCH_GATHERER_DESCRIPTOR: NodeId = NodeId(0x2A1);

/// [`NID_RESERVED2`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Reserved.
pub const NID_RESERVED2: NodeId = NodeId(0x2E1);

/// [`NID_RESERVED3`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Reserved.
pub const NID_RESERVED3: NodeId = NodeId(0x301);

/// [`NID_SEARCH_GATHERER_FOLDER_QUEUE`](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/0510ece4-6853-4bef-8cc8-8df3468e3ff1):
/// Search Gatherer Folder Queue (section [2.4.8.5.3](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/5dd87c45-5f2d-4945-b7e3-2612bd1a94d3)).
pub const NID_SEARCH_GATHERER_FOLDER_QUEUE: NodeId = NodeId(0x321);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nid_index_overflow() {
        let Err(NdbError::InvalidNodeIndex(value)) =
            NodeId::new(NodeIdType::HeapNode, MAX_NODE_INDEX + 1)
        else {
            panic!("NodeId should be out of range");
        };
        assert_eq!(value, MAX_NODE_INDEX + 1);
    }
}
