//! ## [Node Database (NDB) Layer](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/e4efaad0-1876-446e-9d34-bb921588f924)

#![allow(dead_code, unused_imports)]

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::{
    fs::File,
    io::{self, Read, Seek, Write},
    path::Path,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NdbError {
    #[error("Invalid nidType: 0x{0:0X}")]
    InvalidNodeIdType(u8),
    #[error("Invalid nidIndex: 0x{0:0X}")]
    InvalidNodeIndex(u32),
    #[error("Invalid bidIndex: 0x{0:0X}")]
    InvalidUnicodeBlockIndex(u64),
    #[error("Invalid bidIndex: 0x{0:0X}")]
    InvalidAnsiBlockIndex(u32),
}

pub type NdbResult<T> = Result<T, NdbError>;

pub struct PstFile {
    file: File,
}

impl PstFile {
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            file: File::open(path)?,
        })
    }
}

/// `nidType`
///
/// ### See also
/// [NodeId]
#[repr(u8)]
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

/// [NID (Node ID)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/18d7644e-cb33-4e11-95c0-34d8a84fbff6)
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

    pub fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u32::<LittleEndian>()?;
        Ok(Self(value))
    }

    pub fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.0)
    }

    pub fn id_type(&self) -> Result<NodeIdType, NdbError> {
        let nid_type = self.0 & 0x1F;
        NodeIdType::try_from(nid_type as u8)
    }

    pub fn index(&self) -> u32 {
        self.0 >> 5
    }
}

/// [BID (Block ID)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/d3155aa1-ccdd-4dee-a0a9-5363ccca5352)
pub trait BlockId: Sized {
    type Index: Copy;

    fn new(is_internal: bool, index: Self::Index) -> NdbResult<Self>;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
    fn is_internal(&self) -> bool;
    fn index(&self) -> Self::Index;
}

pub const MAX_UNICODE_BLOCK_INDEX: u64 = 1_u64.rotate_right(2) - 1;

pub struct UnicodeBlockId(u64);

impl BlockId for UnicodeBlockId {
    type Index = u64;

    fn new(is_internal: bool, index: u64) -> NdbResult<Self> {
        let is_internal = if is_internal { 0x2 } else { 0x0 };

        let shifted_index = index.rotate_left(2);
        if shifted_index & 0x3 != 0 {
            return Err(NdbError::InvalidUnicodeBlockIndex(index));
        };

        Ok(Self(shifted_index | is_internal))
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u64::<LittleEndian>()?;
        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u64::<LittleEndian>(self.0)
    }

    fn is_internal(&self) -> bool {
        self.0 & 0x2 == 0x2
    }

    fn index(&self) -> u64 {
        self.0 >> 2
    }
}

pub const MAX_ANSI_BLOCK_INDEX: u32 = 1_u32.rotate_right(2) - 1;

pub struct AnsiBlockId(u32);

impl BlockId for AnsiBlockId {
    type Index = u32;

    fn new(is_internal: bool, index: u32) -> NdbResult<Self> {
        let is_internal = if is_internal { 0x2 } else { 0x0 };

        let shifted_index = index.rotate_left(2);
        if shifted_index & 0x3 != 0 {
            return Err(NdbError::InvalidAnsiBlockIndex(index));
        };

        Ok(Self(shifted_index | is_internal))
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u32::<LittleEndian>()?;
        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.0)
    }

    fn is_internal(&self) -> bool {
        self.0 & 0x2 == 0x2
    }

    fn index(&self) -> u32 {
        self.0 >> 2
    }
}

/// [IB (Byte Index)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/7d53d413-b492-4483-b624-4e2fa2a08cf3)
pub trait ByteIndex: Sized {
    type Index: Copy;

    fn new(index: Self::Index) -> Self;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
    fn index(&self) -> Self::Index;
}

pub struct UnicodeByteIndex(u64);

impl ByteIndex for UnicodeByteIndex {
    type Index = u64;

    fn new(index: u64) -> Self {
        Self(index)
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u64::<LittleEndian>()?;
        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u64::<LittleEndian>(self.0)
    }

    fn index(&self) -> u64 {
        self.0
    }
}

pub struct AnsiByteIndex(u32);

impl ByteIndex for AnsiByteIndex {
    type Index = u32;

    fn new(index: u32) -> Self {
        Self(index)
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u32::<LittleEndian>()?;
        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.0)
    }

    fn index(&self) -> u32 {
        self.0
    }
}

/// [BREF](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/844a5ebf-488a-45fd-8fce-92a84d8e24a3)
pub trait BlockRef: Sized {
    type Block: BlockId;
    type Index: ByteIndex;

    fn new(block: Self::Block, index: Self::Index) -> Self;
    fn block(&self) -> &Self::Block;
    fn index(&self) -> &Self::Index;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let block = Self::Block::read(f)?;
        let index = Self::Index::read(f)?;
        Ok(Self::new(block, index))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.block().write(f)?;
        self.index().write(f)
    }
}

pub struct UnicodeBlockRef {
    block: UnicodeBlockId,
    index: UnicodeByteIndex,
}

impl BlockRef for UnicodeBlockRef {
    type Block = UnicodeBlockId;
    type Index = UnicodeByteIndex;

    fn new(block: UnicodeBlockId, index: UnicodeByteIndex) -> Self {
        Self { block, index }
    }

    fn block(&self) -> &UnicodeBlockId {
        &self.block
    }

    fn index(&self) -> &UnicodeByteIndex {
        &self.index
    }
}

pub struct AnsiBlockRef {
    block: AnsiBlockId,
    index: AnsiByteIndex,
}

impl BlockRef for AnsiBlockRef {
    type Block = AnsiBlockId;
    type Index = AnsiByteIndex;

    fn new(block: AnsiBlockId, index: AnsiByteIndex) -> Self {
        Self { block, index }
    }

    fn block(&self) -> &AnsiBlockId {
        &self.block
    }

    fn index(&self) -> &AnsiByteIndex {
        &self.index
    }
}

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

    #[test]
    fn test_unicode_bid_index_overflow() {
        let Err(NdbError::InvalidUnicodeBlockIndex(value)) =
            UnicodeBlockId::new(false, MAX_UNICODE_BLOCK_INDEX + 1)
        else {
            panic!("UnicodeBlockId should be out of range");
        };
        assert_eq!(value, MAX_UNICODE_BLOCK_INDEX + 1);
    }

    #[test]
    fn test_ansi_bid_index_overflow() {
        let Err(NdbError::InvalidAnsiBlockIndex(value)) =
            AnsiBlockId::new(false, MAX_ANSI_BLOCK_INDEX + 1)
        else {
            panic!("AnsiBlockId should be out of range");
        };
        assert_eq!(value, MAX_ANSI_BLOCK_INDEX + 1);
    }
}
