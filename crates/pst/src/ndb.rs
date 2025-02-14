//! ## [Node Database (NDB) Layer](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/e4efaad0-1876-446e-9d34-bb921588f924)

#![allow(dead_code)]

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use core::mem;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use thiserror::Error;

use crate::{block_sig::compute_sig, crc::compute_crc};

#[derive(Error, Debug)]
pub enum NdbError {
    #[error("Invalid nidType: 0x{0:02X}")]
    InvalidNodeIdType(u8),
    #[error("Invalid nidIndex: 0x{0:08X}")]
    InvalidNodeIndex(u32),
    #[error("Invalid bidIndex: 0x{0:016X}")]
    InvalidUnicodeBlockIndex(u64),
    #[error("Invalid bidIndex: 0x{0:08X}")]
    InvalidAnsiBlockIndex(u32),
    #[error("Invalid ROOT fAMapValid: 0x{0:02X}")]
    InvalidAmapStatus(u8),
    #[error("Invalid HEADER wVer: 0x{0:04X}")]
    InvalidNdbVersion(u16),
    #[error("Invalid HEADER bCryptMethod: 0x{0:02X}")]
    InvalidNdbCryptMethod(u8),
    #[error("Invalid HEADER dwMagic: 0x{0:08X}")]
    InvalidNdbHeaderMagicValue(u32),
    #[error("Invalid HEADER dwCRCPartial: 0x{0:08X}")]
    InvalidNdbHeaderPartialCrc(u32),
    #[error("Invalid HEADER wMagicClient: 0x{0:04X}")]
    InvalidNdbHeaderMagicClientValue(u16),
    #[error("Invalid HEADER dwCRCFull: 0x{0:08X}")]
    InvalidNdbHeaderFullCrc(u32),
    #[error("ANSI PST version: 0x{0:04X}")]
    AnsiPstVersion(u16),
    #[error("Invalid HEADER wVerClient: 0x{0:04X}")]
    InvalidNdbHeaderClientVersion(u16),
    #[error("Invalid HEADER bPlatformCreate: 0x{0:02X}")]
    InvalidNdbHeaderPlatformCreate(u8),
    #[error("Invalid HEADER bPlatformAccess: 0x{0:02X}")]
    InvalidNdbHeaderPlatformAccess(u8),
    #[error("Invalid HEADER qwUnused: 0x{0:016X}")]
    InvalidNdbHeaderUnusedValue(u64),
    #[error("Invalid HEADER dwAlign: 0x{0:08X}")]
    InvalidNdbHeaderAlignValue(u32),
    #[error("Invalid HEADER bSentinel: 0x{0:02X}")]
    InvalidNdbHeaderSentinelValue(u8),
    #[error("Invalid HEADER rgbReserved: 0x{0:04X}")]
    InvalidNdbHeaderReservedValue(u16),
    #[error("Unicode PST version: 0x{0:04X}")]
    UnicodePstVersion(u16),
    #[error("Invalid HEADER rgbReserved, ullReserved, dwReserved")]
    InvalidNdbHeaderAnsiReservedBytes,
    #[error("Mismatch between PAGETRAILER ptype and ptypeRepeat: (0x{0:02X}, 0x{1:02X})")]
    MismatchPageTypeRepeat(u8, u8),
    #[error("Invalid PAGETRAILER ptype: 0x{0:02X}")]
    InvalidPageType(u8),
    #[error("Invalid PAGETRAILER ptype: {0:?}")]
    UnexpectedPageType(PageType),
    #[error("Invalid PAGETRAILER dwCRC: 0x{0:08X}")]
    InvalidPageCrc(u32),
    #[error("Invalid ANSI map page dwPadding: 0x{0:08X}")]
    InvalidAnsiMapPagePadding(u32),
    #[error("Invalid DLISTPAGEENT dwPageNum: 0x{0:08X}")]
    InvalidDensityListEntryPageNumber(u32),
    #[error("Invalid DLISTPAGEENT dwFreeSlots: 0x{0:04X}")]
    InvalidDensityListEntryFreeSlots(u16),
    #[error("Invalid DLISTPAGE cbEntDList: 0x{0:016X}")]
    InvalidDensityListEntryCount(usize),
    #[error("Invalid DLISTPAGE rgPadding")]
    InvalidDensityListPadding,
}

impl From<NdbError> for io::Error {
    fn from(err: NdbError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

pub type NdbResult<T> = Result<T, NdbError>;

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

/// [NID (Node ID)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/18d7644e-cb33-4e11-95c0-34d8a84fbff6)
#[derive(Clone, Copy, Debug)]
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
pub trait BlockId: Sized + Copy {
    type Index: Copy;

    fn new(is_internal: bool, index: Self::Index) -> NdbResult<Self>;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
    fn is_internal(&self) -> bool;
    fn index(&self) -> Self::Index;
}

pub const MAX_UNICODE_BLOCK_INDEX: u64 = 1_u64.rotate_right(2) - 1;

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
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

/// `fAMapValid`
///
/// ### See also
/// [Root]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum AmapStatus {
    /// `INVALID_AMAP`: One or more AMaps in the PST are INVALID
    #[default]
    Invalid = 0x00,
    /// `VALID_AMAP1`: Deprecated. Implementations SHOULD NOT use this value. The AMaps are VALID.
    Valid1 = 0x01,
    /// `VALID_AMAP2`: The AMaps are VALID.
    Valid2 = 0x02,
}

impl TryFrom<u8> for AmapStatus {
    type Error = NdbError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(AmapStatus::Invalid),
            0x01 => Ok(AmapStatus::Valid1),
            0x02 => Ok(AmapStatus::Valid2),
            _ => Err(NdbError::InvalidAmapStatus(value)),
        }
    }
}

impl From<AmapStatus> for bool {
    fn from(status: AmapStatus) -> bool {
        status != AmapStatus::Invalid
    }
}

/// [ROOT](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/32ce8c94-4757-46c8-a169-3fd21abee584)
pub trait Root: Sized {
    type Index: ByteIndex;
    type BTreeRef: BlockRef;

    fn new(
        file_eof_index: Self::Index,
        amap_last_index: Self::Index,
        amap_free_size: Self::Index,
        pmap_free_size: Self::Index,
        node_btree: Self::BTreeRef,
        block_btree: Self::BTreeRef,
        amap_is_valid: AmapStatus,
    ) -> Self;

    fn file_eof_index(&self) -> &Self::Index;
    fn amap_last_index(&self) -> &Self::Index;
    fn amap_free_size(&self) -> &Self::Index;
    fn pmap_free_size(&self) -> &Self::Index;
    fn node_btree(&self) -> &Self::BTreeRef;
    fn block_btree(&self) -> &Self::BTreeRef;
    fn amap_is_valid(&self) -> AmapStatus;
}

trait RootReadWrite: Root {
    fn load_reserved(&mut self, reserved1: u32, reserved2: u8, reserved3: u16);

    fn reserved1(&self) -> u32;
    fn reserved2(&self) -> u8;
    fn reserved3(&self) -> u16;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let reserved1 = f.read_u32::<LittleEndian>()?;
        let file_eof_index = Self::Index::read(f)?;
        let amap_last_index = Self::Index::read(f)?;
        let amap_free_size = Self::Index::read(f)?;
        let pmap_free_size = Self::Index::read(f)?;
        let node_btree = Self::BTreeRef::read(f)?;
        let block_btree = Self::BTreeRef::read(f)?;
        let amap_is_valid = AmapStatus::try_from(f.read_u8()?).unwrap_or(AmapStatus::Invalid);
        let reserved2 = f.read_u8()?;
        let reserved3 = f.read_u16::<LittleEndian>()?;
        let mut root = Self::new(
            file_eof_index,
            amap_last_index,
            amap_free_size,
            pmap_free_size,
            node_btree,
            block_btree,
            amap_is_valid,
        );
        root.load_reserved(reserved1, reserved2, reserved3);
        Ok(root)
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.reserved1())?;
        self.file_eof_index().write(f)?;
        self.amap_last_index().write(f)?;
        self.amap_free_size().write(f)?;
        self.pmap_free_size().write(f)?;
        self.node_btree().write(f)?;
        self.block_btree().write(f)?;
        f.write_u8(self.amap_is_valid() as u8)?;
        f.write_u8(self.reserved2())?;
        f.write_u16::<LittleEndian>(self.reserved3())
    }
}

pub struct UnicodeRoot {
    reserved1: u32,
    file_eof_index: UnicodeByteIndex,
    amap_last_index: UnicodeByteIndex,
    amap_free_size: UnicodeByteIndex,
    pmap_free_size: UnicodeByteIndex,
    node_btree: UnicodeBlockRef,
    block_btree: UnicodeBlockRef,
    amap_is_valid: AmapStatus,
    reserved2: u8,
    reserved3: u16,
}

impl Root for UnicodeRoot {
    type Index = UnicodeByteIndex;
    type BTreeRef = UnicodeBlockRef;

    fn new(
        file_eof_index: UnicodeByteIndex,
        amap_last_index: UnicodeByteIndex,
        amap_free_size: UnicodeByteIndex,
        pmap_free_size: UnicodeByteIndex,
        node_btree: UnicodeBlockRef,
        block_btree: UnicodeBlockRef,
        amap_is_valid: AmapStatus,
    ) -> Self {
        Self {
            reserved1: Default::default(),
            file_eof_index,
            amap_last_index,
            amap_free_size,
            pmap_free_size,
            node_btree,
            block_btree,
            amap_is_valid,
            reserved2: Default::default(),
            reserved3: Default::default(),
        }
    }

    fn file_eof_index(&self) -> &UnicodeByteIndex {
        &self.file_eof_index
    }

    fn amap_last_index(&self) -> &UnicodeByteIndex {
        &self.amap_last_index
    }

    fn amap_free_size(&self) -> &UnicodeByteIndex {
        &self.amap_free_size
    }

    fn pmap_free_size(&self) -> &UnicodeByteIndex {
        &self.pmap_free_size
    }

    fn node_btree(&self) -> &UnicodeBlockRef {
        &self.node_btree
    }

    fn block_btree(&self) -> &UnicodeBlockRef {
        &self.block_btree
    }

    fn amap_is_valid(&self) -> AmapStatus {
        self.amap_is_valid
    }
}

impl RootReadWrite for UnicodeRoot {
    fn load_reserved(&mut self, reserved1: u32, reserved2: u8, reserved3: u16) {
        self.reserved1 = reserved1;
        self.reserved2 = reserved2;
        self.reserved3 = reserved3;
    }

    fn reserved1(&self) -> u32 {
        self.reserved1
    }

    fn reserved2(&self) -> u8 {
        self.reserved2
    }

    fn reserved3(&self) -> u16 {
        self.reserved3
    }
}

pub struct AnsiRoot {
    file_eof_index: AnsiByteIndex,
    amap_last_index: AnsiByteIndex,
    amap_free_size: AnsiByteIndex,
    pmap_free_size: AnsiByteIndex,
    node_btree: AnsiBlockRef,
    block_btree: AnsiBlockRef,
    amap_is_valid: AmapStatus,
    reserved1: u32,
    reserved2: u8,
    reserved3: u16,
}

impl Root for AnsiRoot {
    type Index = AnsiByteIndex;
    type BTreeRef = AnsiBlockRef;

    fn new(
        file_eof_index: AnsiByteIndex,
        amap_last_index: AnsiByteIndex,
        amap_free_size: AnsiByteIndex,
        pmap_free_size: AnsiByteIndex,
        node_btree: AnsiBlockRef,
        block_btree: AnsiBlockRef,
        amap_is_valid: AmapStatus,
    ) -> Self {
        Self {
            reserved1: Default::default(),
            file_eof_index,
            amap_last_index,
            amap_free_size,
            pmap_free_size,
            node_btree,
            block_btree,
            amap_is_valid,
            reserved2: Default::default(),
            reserved3: Default::default(),
        }
    }

    fn file_eof_index(&self) -> &AnsiByteIndex {
        &self.file_eof_index
    }

    fn amap_last_index(&self) -> &AnsiByteIndex {
        &self.amap_last_index
    }

    fn amap_free_size(&self) -> &AnsiByteIndex {
        &self.amap_free_size
    }

    fn pmap_free_size(&self) -> &AnsiByteIndex {
        &self.pmap_free_size
    }

    fn node_btree(&self) -> &AnsiBlockRef {
        &self.node_btree
    }

    fn block_btree(&self) -> &AnsiBlockRef {
        &self.block_btree
    }

    fn amap_is_valid(&self) -> AmapStatus {
        self.amap_is_valid
    }
}

impl RootReadWrite for AnsiRoot {
    fn load_reserved(&mut self, reserved1: u32, reserved2: u8, reserved3: u16) {
        self.reserved1 = reserved1;
        self.reserved2 = reserved2;
        self.reserved3 = reserved3;
    }

    fn reserved1(&self) -> u32 {
        self.reserved1
    }

    fn reserved2(&self) -> u8 {
        self.reserved2
    }

    fn reserved3(&self) -> u16 {
        self.reserved3
    }
}

/// `dwMagic`
///
/// ### See also
/// [Header]
const HEADER_MAGIC: u32 = u32::from_be_bytes(*b"NDB!");

const HEADER_MAGIC_CLIENT: u16 = u16::from_be_bytes(*b"MS");

/// `wVer`
///
/// ### See also
/// [Header]
#[repr(u16)]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum NdbVersion {
    Ansi = 15,
    #[default]
    Unicode = 23,
}

impl TryFrom<u16> for NdbVersion {
    type Error = NdbError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            14..=15 => Ok(NdbVersion::Ansi),
            23 => Ok(NdbVersion::Unicode),
            _ => Err(NdbError::InvalidNdbVersion(value)),
        }
    }
}

const NDB_CLIENT_VERSION: u16 = 19;
const NDB_PLATFORM_CREATE: u8 = 0x01;
const NDB_PLATFORM_ACCESS: u8 = 0x01;
const NDB_DEFAULT_NIDS: [u32; 32] = [
    0x400 << 5,
    0x400 << 5 | 0x01,
    0x400 << 5 | 0x02,
    0x4000 << 5 | 0x03,
    0x10000 << 5 | 0x04,
    0x400 << 5 | 0x05,
    0x400 << 5 | 0x06,
    0x400 << 5 | 0x07,
    0x8000 << 5 | 0x08,
    0x400 << 5 | 0x09,
    0x400 << 5 | 0x0A,
    0x400 << 5 | 0x0B,
    0x400 << 5 | 0x0C,
    0x400 << 5 | 0x0D,
    0x400 << 5 | 0x0E,
    0x400 << 5 | 0x0F,
    0x400 << 5 | 0x10,
    0x400 << 5 | 0x11,
    0x400 << 5 | 0x12,
    0x400 << 5 | 0x13,
    0x400 << 5 | 0x14,
    0x400 << 5 | 0x15,
    0x400 << 5 | 0x16,
    0x400 << 5 | 0x17,
    0x400 << 5 | 0x18,
    0x400 << 5 | 0x19,
    0x400 << 5 | 0x1A,
    0x400 << 5 | 0x1B,
    0x400 << 5 | 0x1C,
    0x400 << 5 | 0x1D,
    0x400 << 5 | 0x1E,
    0x400 << 5 | 0x1F,
];
const NDB_SENTINEL: u8 = 0x80;

/// `bCryptMethod`
///
/// ### See also
/// [Header]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum NdbCryptMethod {
    /// `NDB_CRYPT_NONE`: Data blocks are not encoded
    #[default]
    None = 0x00,
    /// `NDB_CRYPT_PERMUTE`: Encoded with the [Permutation algorithm](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/5faf4800-645d-49d1-9457-2ac40eb467bd)
    Permute = 0x01,
    /// `NDB_CRYPT_CYCLIC`: Encoded with the [Cyclic algorithm](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/9979fc01-0a3e-496f-900f-a6a867951f23)
    Cyclic = 0x02,
}

impl TryFrom<u8> for NdbCryptMethod {
    type Error = NdbError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(NdbCryptMethod::None),
            0x01 => Ok(NdbCryptMethod::Permute),
            0x02 => Ok(NdbCryptMethod::Cyclic),
            _ => Err(NdbError::InvalidNdbCryptMethod(value)),
        }
    }
}

type HeaderCrcBlock = [u8; 471];

/// [HEADER](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/c9876f5a-664b-46a3-9887-ba63f113abf5)
pub trait Header: Sized {
    type Root: Root;

    fn new(root: Self::Root, crypt_method: NdbCryptMethod) -> Self;
    fn version(&self) -> NdbVersion;
    fn root(&self) -> &Self::Root;
    fn root_mut(&mut self) -> &mut Self::Root;
}

pub struct UnicodeHeader {
    next_page: UnicodeBlockId,
    unique: u32,
    nids: [u32; 32],
    root: UnicodeRoot,
    crypt_method: NdbCryptMethod,
    next_block: UnicodeBlockId,

    reserved1: u32,
    reserved2: u32,
    unused: u64,
    reserved3: [u8; 36],
}

impl Header for UnicodeHeader {
    type Root = UnicodeRoot;

    fn new(root: UnicodeRoot, crypt_method: NdbCryptMethod) -> Self {
        Self {
            next_page: UnicodeBlockId(0),
            unique: 0,
            nids: NDB_DEFAULT_NIDS,
            root,
            crypt_method,
            next_block: UnicodeBlockId(0),
            reserved1: 0,
            reserved2: 0,
            unused: 0,
            reserved3: [0; 36],
        }
    }

    fn version(&self) -> NdbVersion {
        NdbVersion::Unicode
    }

    fn root(&self) -> &UnicodeRoot {
        &self.root
    }

    fn root_mut(&mut self) -> &mut UnicodeRoot {
        &mut self.root
    }
}

impl UnicodeHeader {
    pub fn read(f: &mut dyn Read) -> io::Result<Self> {
        // dwMagic
        let magic = f.read_u32::<LittleEndian>()?;
        if magic != HEADER_MAGIC {
            return Err(NdbError::InvalidNdbHeaderMagicValue(magic).into());
        }

        // dwCRCPartial
        let crc_partial = f.read_u32::<LittleEndian>()?;

        let mut crc_data = [0_u8; 516];
        f.read_exact(&mut crc_data[..471])?;
        if crc_partial != compute_crc(0, &crc_data[..471]) {
            return Err(NdbError::InvalidNdbHeaderPartialCrc(crc_partial).into());
        }

        let mut cursor = Cursor::new(crc_data);

        // wMagicClient
        let magic = cursor.read_u16::<LittleEndian>()?;
        if magic != HEADER_MAGIC_CLIENT {
            return Err(NdbError::InvalidNdbHeaderMagicClientValue(magic).into());
        }

        // wVer
        let version = NdbVersion::try_from(cursor.read_u16::<LittleEndian>()?)?;
        if version != NdbVersion::Unicode {
            return Err(NdbError::AnsiPstVersion(version as u16).into());
        }

        let mut crc_data = cursor.into_inner();
        f.read_exact(&mut crc_data[471..])?;

        // dwCRCFull
        let crc_full = f.read_u32::<LittleEndian>()?;
        if crc_full != compute_crc(0, &crc_data) {
            return Err(NdbError::InvalidNdbHeaderFullCrc(crc_full).into());
        }

        let mut cursor = Cursor::new(crc_data);
        cursor.seek(SeekFrom::Start(4))?;

        // wVerClient
        let version = cursor.read_u16::<LittleEndian>()?;
        if version != NDB_CLIENT_VERSION {
            return Err(NdbError::InvalidNdbHeaderClientVersion(version).into());
        }

        // bPlatformCreate
        let platform_create = cursor.read_u8()?;
        if platform_create != NDB_PLATFORM_CREATE {
            return Err(NdbError::InvalidNdbHeaderPlatformCreate(platform_create).into());
        }

        // bPlatformAccess
        let platform_access = cursor.read_u8()?;
        if platform_access != NDB_PLATFORM_ACCESS {
            return Err(NdbError::InvalidNdbHeaderPlatformAccess(platform_access).into());
        }

        // dwReserved1
        let reserved1 = cursor.read_u32::<LittleEndian>()?;

        // dwReserved2
        let reserved2 = cursor.read_u32::<LittleEndian>()?;

        // bidUnused
        let unused = cursor.read_u64::<LittleEndian>()?;

        // bidNextP
        let next_page = UnicodeBlockId::read(&mut cursor)?;

        // dwUnique
        let unique = cursor.read_u32::<LittleEndian>()?;

        // rgnid
        let mut nids = [0_u32; 32];
        for nid in nids.iter_mut() {
            *nid = cursor.read_u32::<LittleEndian>()?;
        }

        // qwUnused
        {
            let unused = cursor.read_u64::<LittleEndian>()?;
            if unused != 0 {
                return Err(NdbError::InvalidNdbHeaderUnusedValue(unused).into());
            }
        }

        // root
        let root = UnicodeRoot::read(&mut cursor)?;

        // dwAlign
        let align = cursor.read_u32::<LittleEndian>()?;
        if align != 0 {
            return Err(NdbError::InvalidNdbHeaderAlignValue(align).into());
        }

        // rgbFM
        cursor.seek(SeekFrom::Current(128))?;

        // rgbFP
        cursor.seek(SeekFrom::Current(128))?;

        // bSentinel
        let sentinel = cursor.read_u8()?;
        if sentinel != NDB_SENTINEL {
            return Err(NdbError::InvalidNdbHeaderSentinelValue(sentinel).into());
        }

        // bCryptMethod
        let crypt_method = NdbCryptMethod::try_from(cursor.read_u8()?)?;

        // rgbReserved
        let reserved = cursor.read_u16::<LittleEndian>()?;
        if reserved != 0 {
            return Err(NdbError::InvalidNdbHeaderReservedValue(reserved).into());
        }

        // bidNextB
        let next_block = UnicodeBlockId::read(&mut cursor)?;

        // rgbReserved2, bReserved, rgbReserved3 (total 36 bytes)
        let mut reserved3 = [0_u8; 36];
        f.read_exact(&mut reserved3)?;

        Ok(Self {
            next_page,
            unique,
            nids,
            root,
            crypt_method,
            next_block,
            reserved1,
            reserved2,
            unused,
            reserved3,
        })
    }

    pub fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        let mut cursor = Cursor::new([0_u8; 516]);
        // wMagicClient
        cursor.write_u16::<LittleEndian>(HEADER_MAGIC_CLIENT)?;
        // wVer
        cursor.write_u16::<LittleEndian>(NdbVersion::Unicode as u16)?;
        // wVerClient
        cursor.write_u16::<LittleEndian>(NDB_CLIENT_VERSION)?;
        // bPlatformCreate
        cursor.write_u8(NDB_PLATFORM_CREATE)?;
        // bPlatformAccess
        cursor.write_u8(NDB_PLATFORM_ACCESS)?;
        // dwReserved1
        cursor.write_u32::<LittleEndian>(self.reserved1)?;
        // dwReserved2
        cursor.write_u32::<LittleEndian>(self.reserved2)?;
        // bidUnused
        cursor.write_u64::<LittleEndian>(self.unused)?;
        // bidNextP
        self.next_page.write(&mut cursor)?;
        // dwUnique
        cursor.write_u32::<LittleEndian>(self.unique)?;
        // rgnid
        for nid in self.nids.iter() {
            cursor.write_u32::<LittleEndian>(*nid)?;
        }
        // qwUnused
        cursor.write_u64::<LittleEndian>(0)?;
        // root
        self.root.write(&mut cursor)?;
        // dwAlign
        cursor.write_u32::<LittleEndian>(0)?;
        // rgbFM
        cursor.write_all(&[0xFF; 128])?;
        // rgbFP
        cursor.write_all(&[0xFF; 128])?;
        // bSentinel
        cursor.write_u8(NDB_SENTINEL)?;
        // bCryptMethod
        cursor.write_u8(self.crypt_method as u8)?;
        // rgbReserved
        cursor.write_u16::<LittleEndian>(0)?;
        // bidNextB
        self.next_block.write(&mut cursor)?;

        let crc_data = cursor.into_inner();
        let crc_partial = compute_crc(0, &crc_data[..471]);
        let crc_full = compute_crc(0, &crc_data);

        // dwMagic
        f.write_u32::<LittleEndian>(HEADER_MAGIC)?;
        // dwCRCPartial
        f.write_u32::<LittleEndian>(crc_partial)?;

        f.write_all(&crc_data)?;

        // dwCRCFull
        f.write_u32::<LittleEndian>(crc_full)?;

        // rgbReserved2, bReserved, rgbReserved3 (total 36 bytes)
        f.write_all(&self.reserved3)
    }
}

pub struct AnsiHeader {
    next_block: AnsiBlockId,
    next_page: AnsiBlockId,
    unique: u32,
    nids: [u32; 32],
    root: AnsiRoot,
    crypt_method: NdbCryptMethod,

    reserved1: u32,
    reserved2: u32,
    reserved3: [u8; 36],
}

impl Header for AnsiHeader {
    type Root = AnsiRoot;

    fn new(root: AnsiRoot, crypt_method: NdbCryptMethod) -> Self {
        Self {
            next_block: AnsiBlockId(0),
            next_page: AnsiBlockId(0),
            unique: 0,
            nids: NDB_DEFAULT_NIDS,
            root,
            crypt_method,
            reserved1: 0,
            reserved2: 0,
            reserved3: [0; 36],
        }
    }

    fn version(&self) -> NdbVersion {
        NdbVersion::Ansi
    }

    fn root(&self) -> &AnsiRoot {
        &self.root
    }

    fn root_mut(&mut self) -> &mut Self::Root {
        &mut self.root
    }
}

impl AnsiHeader {
    pub fn read(f: &mut dyn Read) -> io::Result<Self> {
        // dwMagic
        let magic = f.read_u32::<LittleEndian>()?;
        if magic != HEADER_MAGIC {
            return Err(NdbError::InvalidNdbHeaderMagicValue(magic).into());
        }

        // dwCRCPartial
        let crc_partial = f.read_u32::<LittleEndian>()?;

        let mut crc_data = [0_u8; 504];
        f.read_exact(&mut crc_data)?;
        if crc_partial != compute_crc(0, &crc_data[..471]) {
            return Err(NdbError::InvalidNdbHeaderPartialCrc(crc_partial).into());
        }

        let mut cursor = Cursor::new(crc_data);

        // wMagicClient
        let magic = cursor.read_u16::<LittleEndian>()?;
        if magic != HEADER_MAGIC_CLIENT {
            return Err(NdbError::InvalidNdbHeaderMagicClientValue(magic).into());
        }

        // wVer
        let version = NdbVersion::try_from(cursor.read_u16::<LittleEndian>()?)?;
        if version != NdbVersion::Ansi {
            return Err(NdbError::UnicodePstVersion(version as u16).into());
        }

        // wVerClient
        let version = cursor.read_u16::<LittleEndian>()?;
        if version != NDB_CLIENT_VERSION {
            return Err(NdbError::InvalidNdbHeaderClientVersion(version).into());
        }

        // bPlatformCreate
        let platform_create = cursor.read_u8()?;
        if platform_create != NDB_PLATFORM_CREATE {
            return Err(NdbError::InvalidNdbHeaderPlatformCreate(platform_create).into());
        }

        // bPlatformAccess
        let platform_access = cursor.read_u8()?;
        if platform_access != NDB_PLATFORM_ACCESS {
            return Err(NdbError::InvalidNdbHeaderPlatformAccess(platform_access).into());
        }

        // dwReserved1
        let reserved1 = cursor.read_u32::<LittleEndian>()?;

        // dwReserved2
        let reserved2 = cursor.read_u32::<LittleEndian>()?;

        // bidNextB
        let next_block = AnsiBlockId::read(&mut cursor)?;

        // bidNextP
        let next_page = AnsiBlockId::read(&mut cursor)?;

        // dwUnique
        let unique = cursor.read_u32::<LittleEndian>()?;

        // rgnid
        let mut nids = [0_u32; 32];
        for nid in nids.iter_mut() {
            *nid = cursor.read_u32::<LittleEndian>()?;
        }

        // root
        let root = AnsiRoot::read(&mut cursor)?;

        // rgbFM
        cursor.seek(SeekFrom::Current(128))?;

        // rgbFP
        cursor.seek(SeekFrom::Current(128))?;

        // bSentinel
        let sentinel = cursor.read_u8()?;
        if sentinel != NDB_SENTINEL {
            return Err(NdbError::InvalidNdbHeaderSentinelValue(sentinel).into());
        }

        // bCryptMethod
        let crypt_method = NdbCryptMethod::try_from(cursor.read_u8()?)?;

        // rgbReserved
        let reserved = cursor.read_u16::<LittleEndian>()?;
        if reserved != 0 {
            return Err(NdbError::InvalidNdbHeaderReservedValue(reserved).into());
        }

        // rgbReserved, ullReserved, dwReserved (total 14 bytes)
        let mut reserved = [0_u8; 14];
        cursor.read_exact(&mut reserved)?;
        if reserved != [0; 14] {
            return Err(NdbError::InvalidNdbHeaderAnsiReservedBytes.into());
        }

        // rgbReserved2, bReserved, rgbReserved3 (total 36 bytes)
        let mut reserved3 = [0_u8; 36];
        f.read_exact(&mut reserved3)?;

        Ok(Self {
            next_page,
            unique,
            nids,
            root,
            crypt_method,
            next_block,
            reserved1,
            reserved2,
            reserved3,
        })
    }

    pub fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        let mut cursor = Cursor::new([0_u8; 504]);
        // wMagicClient
        cursor.write_u16::<LittleEndian>(HEADER_MAGIC_CLIENT)?;
        // wVer
        cursor.write_u16::<LittleEndian>(NdbVersion::Ansi as u16)?;
        // wVerClient
        cursor.write_u16::<LittleEndian>(NDB_CLIENT_VERSION)?;
        // bPlatformCreate
        cursor.write_u8(NDB_PLATFORM_CREATE)?;
        // bPlatformAccess
        cursor.write_u8(NDB_PLATFORM_ACCESS)?;
        // dwReserved1
        cursor.write_u32::<LittleEndian>(self.reserved1)?;
        // dwReserved2
        cursor.write_u32::<LittleEndian>(self.reserved2)?;
        // bidNextB
        self.next_block.write(&mut cursor)?;
        // bidNextP
        self.next_page.write(&mut cursor)?;
        // dwUnique
        cursor.write_u32::<LittleEndian>(self.unique)?;
        // rgnid
        for nid in self.nids.iter() {
            cursor.write_u32::<LittleEndian>(*nid)?;
        }
        // root
        self.root.write(&mut cursor)?;
        // rgbFM
        cursor.write_all(&[0xFF; 128])?;
        // rgbFP
        cursor.write_all(&[0xFF; 128])?;
        // bSentinel
        cursor.write_u8(NDB_SENTINEL)?;
        // bCryptMethod
        cursor.write_u8(self.crypt_method as u8)?;
        // rgbReserved
        cursor.write_u16::<LittleEndian>(0)?;

        let crc_data = cursor.into_inner();
        let crc_partial = compute_crc(0, &crc_data[..471]);

        // dwMagic
        f.write_u32::<LittleEndian>(HEADER_MAGIC)?;
        // dwCRCPartial
        f.write_u32::<LittleEndian>(crc_partial)?;

        f.write_all(&crc_data)?;

        // rgbReserved2, bReserved, rgbReserved3 (total 36 bytes)
        f.write_all(&self.reserved3)
    }
}

/// `ptype`
///
/// ### See also
/// [PageTrailer]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PageType {
    /// `ptypeBBT`: Block BTree page
    BlockBTree = 0x80,
    /// `ptypeNBT`: Node BTree page
    NodeBTree = 0x81,
    /// `ptypeFMap`: Free Map page
    FreeMap = 0x82,
    /// `ptypePMap`: Allocation Page Map page
    AllocationPageMap = 0x83,
    /// `ptypeAMap`: Allocation Map page
    AllocationMap = 0x84,
    /// `ptypeFPMap`: Free Page Map page
    FreePageMap = 0x85,
    /// `ptypeDL`: Density List page
    DensityList = 0x86,
}

impl TryFrom<u8> for PageType {
    type Error = NdbError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x80 => Ok(PageType::BlockBTree),
            0x81 => Ok(PageType::NodeBTree),
            0x82 => Ok(PageType::FreeMap),
            0x83 => Ok(PageType::AllocationPageMap),
            0x84 => Ok(PageType::AllocationMap),
            0x85 => Ok(PageType::FreePageMap),
            0x86 => Ok(PageType::DensityList),
            _ => Err(NdbError::InvalidPageType(value)),
        }
    }
}

impl PageType {
    pub fn signature(&self, index: u32, block_id: u32) -> u16 {
        match self {
            PageType::BlockBTree | PageType::NodeBTree | PageType::DensityList => {
                compute_sig(index, block_id)
            }
            _ => 0,
        }
    }
}

/// [PAGETRAILER](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/f4ccb38a-930a-4db4-98df-a69c195926ba)
pub trait PageTrailer: Sized {
    type BlockId: BlockId;

    fn new(page_type: PageType, signature: u16, block_id: Self::BlockId, crc: u32) -> Self;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
    fn page_type(&self) -> PageType;
    fn signature(&self) -> u16;
    fn crc(&self) -> u32;
    fn block_id(&self) -> Self::BlockId;
}

pub struct UnicodePageTrailer {
    page_type: PageType,
    signature: u16,
    crc: u32,
    block_id: UnicodeBlockId,
}

impl PageTrailer for UnicodePageTrailer {
    type BlockId = UnicodeBlockId;

    fn new(page_type: PageType, signature: u16, block_id: UnicodeBlockId, crc: u32) -> Self {
        Self {
            page_type,
            block_id,
            signature,
            crc,
        }
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let mut page_type = [0_u8; 2];
        f.read_exact(&mut page_type)?;
        if page_type[0] != page_type[1] {
            return Err(NdbError::MismatchPageTypeRepeat(page_type[0], page_type[1]).into());
        }
        let page_type = PageType::try_from(page_type[0])?;
        let signature = f.read_u16::<LittleEndian>()?;
        let crc = f.read_u32::<LittleEndian>()?;
        let block_id = UnicodeBlockId::read(f)?;

        Ok(Self {
            page_type,
            signature,
            crc,
            block_id,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_all(&[self.page_type as u8; 2])?;
        f.write_u16::<LittleEndian>(self.signature)?;
        f.write_u32::<LittleEndian>(self.crc)?;
        self.block_id.write(f)
    }

    fn page_type(&self) -> PageType {
        self.page_type
    }

    fn signature(&self) -> u16 {
        self.signature
    }

    fn crc(&self) -> u32 {
        self.crc
    }

    fn block_id(&self) -> UnicodeBlockId {
        self.block_id
    }
}

pub struct AnsiPageTrailer {
    page_type: PageType,
    signature: u16,
    block_id: AnsiBlockId,
    crc: u32,
}

impl PageTrailer for AnsiPageTrailer {
    type BlockId = AnsiBlockId;

    fn new(page_type: PageType, signature: u16, block_id: AnsiBlockId, crc: u32) -> Self {
        Self {
            page_type,
            crc,
            block_id,
            signature,
        }
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let mut page_type = [0_u8; 2];
        f.read_exact(&mut page_type)?;
        if page_type[0] != page_type[1] {
            return Err(NdbError::MismatchPageTypeRepeat(page_type[0], page_type[1]).into());
        }
        let page_type = PageType::try_from(page_type[0])?;
        let signature = f.read_u16::<LittleEndian>()?;
        let block_id = AnsiBlockId::read(f)?;
        let crc = f.read_u32::<LittleEndian>()?;

        Ok(Self {
            page_type,
            signature,
            block_id,
            crc,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_all(&[self.page_type as u8; 2])?;
        f.write_u16::<LittleEndian>(self.signature)?;
        self.block_id.write(f)?;
        f.write_u32::<LittleEndian>(self.crc)
    }

    fn page_type(&self) -> PageType {
        self.page_type
    }

    fn signature(&self) -> u16 {
        self.signature
    }

    fn crc(&self) -> u32 {
        self.crc
    }

    fn block_id(&self) -> AnsiBlockId {
        self.block_id
    }
}

pub type MapBits = [u8; 496];

pub trait MapPage: Sized {
    type Trailer: PageTrailer;
    const PAGE_TYPE: u8;

    fn new(amap_bits: MapBits, trailer: Self::Trailer) -> NdbResult<Self>;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
    fn map_bits(&self) -> &MapBits;
    fn trailer(&self) -> &Self::Trailer;
}

pub struct UnicodeMapPage<const P: u8> {
    map_bits: MapBits,
    trailer: UnicodePageTrailer,
}

impl<const P: u8> MapPage for UnicodeMapPage<P> {
    type Trailer = UnicodePageTrailer;
    const PAGE_TYPE: u8 = P;

    fn new(map_bits: MapBits, trailer: UnicodePageTrailer) -> NdbResult<Self> {
        if trailer.page_type() as u8 != Self::PAGE_TYPE {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }
        Ok(Self { map_bits, trailer })
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let mut map_bits = [0_u8; mem::size_of::<MapBits>()];
        f.read_exact(&mut map_bits)?;

        let trailer = UnicodePageTrailer::read(f)?;
        if trailer.page_type() as u8 != Self::PAGE_TYPE {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()).into());
        }

        let crc = compute_crc(0, &map_bits);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidPageCrc(crc).into());
        }

        Ok(Self { map_bits, trailer })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_all(&self.map_bits)?;

        let crc = compute_crc(0, &self.map_bits);
        let trailer = UnicodePageTrailer {
            crc,
            ..self.trailer
        };
        trailer.write(f)
    }

    fn map_bits(&self) -> &MapBits {
        &self.map_bits
    }

    fn trailer(&self) -> &UnicodePageTrailer {
        &self.trailer
    }
}

/// [AMAPPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/43d8f556-2c0e-4976-8ec7-84e57f8b1234)
pub type UnicodeAllocationMapPage = UnicodeMapPage<{ PageType::AllocationMap as u8 }>;
/// [PMAPPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/7e64a91f-cbd1-4a11-90c9-df5789e7d9a1)
pub type UnicodeAllocationPageMapPage = UnicodeMapPage<{ PageType::AllocationPageMap as u8 }>;
/// [FMAPPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/26273ead-797e-4ea6-9b3c-9b9a5c581115)
pub type UnicodeFreeMapPage = UnicodeMapPage<{ PageType::FreeMap as u8 }>;
/// [FPMAPPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/913a72b0-83f6-4c29-8b0b-40967579a534)
pub type UnicodeFreePageMapPage = UnicodeMapPage<{ PageType::FreePageMap as u8 }>;

pub struct AnsiMapPage<const P: u8> {
    map_bits: MapBits,
    trailer: AnsiPageTrailer,
}

impl<const P: u8> MapPage for AnsiMapPage<P> {
    type Trailer = AnsiPageTrailer;
    const PAGE_TYPE: u8 = P;

    fn new(amap_bits: MapBits, trailer: AnsiPageTrailer) -> NdbResult<Self> {
        if trailer.page_type() as u8 != Self::PAGE_TYPE {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }
        Ok(Self {
            map_bits: amap_bits,
            trailer,
        })
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let mut buffer = [0_u8; 500];
        f.read_exact(&mut buffer)?;
        let mut cursor = Cursor::new(buffer);

        let padding = cursor.read_u32::<LittleEndian>()?;
        if padding != 0 {
            return Err(NdbError::InvalidAnsiMapPagePadding(padding).into());
        }

        let mut map_bits = [0_u8; mem::size_of::<MapBits>()];
        cursor.read_exact(&mut map_bits)?;

        let buffer = cursor.into_inner();

        let trailer = AnsiPageTrailer::read(f)?;
        if trailer.page_type() as u8 != Self::PAGE_TYPE {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()).into());
        }

        let crc = compute_crc(0, &buffer);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidPageCrc(crc).into());
        }

        Ok(Self { map_bits, trailer })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        let mut cursor = Cursor::new([0_u8; 500]);

        cursor.write_u32::<LittleEndian>(0)?;
        cursor.write_all(&self.map_bits)?;

        let buffer = cursor.into_inner();
        let crc = compute_crc(0, &buffer);

        f.write_all(&buffer)?;

        let trailer = AnsiPageTrailer {
            crc,
            ..self.trailer
        };
        trailer.write(f)
    }

    fn map_bits(&self) -> &MapBits {
        &self.map_bits
    }

    fn trailer(&self) -> &AnsiPageTrailer {
        &self.trailer
    }
}

/// [AMAPPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/43d8f556-2c0e-4976-8ec7-84e57f8b1234)
pub type AnsiAllocationMapPage = AnsiMapPage<{ PageType::AllocationMap as u8 }>;
/// [PMAPPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/7e64a91f-cbd1-4a11-90c9-df5789e7d9a1)
pub type AnsiAllocationPageMapPage = AnsiMapPage<{ PageType::AllocationPageMap as u8 }>;
/// [FMAPPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/26273ead-797e-4ea6-9b3c-9b9a5c581115)
pub type AnsiFreeMapPage = AnsiMapPage<{ PageType::FreeMap as u8 }>;
/// [FPMAPPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/913a72b0-83f6-4c29-8b0b-40967579a534)
pub type AnsiFreePageMapPage = AnsiMapPage<{ PageType::FreePageMap as u8 }>;

const DENSITY_LIST_ENTRY_PAGE_NUMBER_MASK: u32 = 0x000F_FFFF;
const DENSITY_LIST_ENTRY_FREE_SLOTS_MASK: u32 = 0x0FFF;

/// [DLISTPAGEENT](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/9d3c45b9-a415-446c-954f-b1b473dbb415)
#[derive(Copy, Clone, Debug)]
pub struct DensityListPageEntry(u32);

impl DensityListPageEntry {
    pub fn new(page: u32, free_slots: u16) -> NdbResult<Self> {
        if page & !0x000F_FFFF != 0 {
            return Err(NdbError::InvalidDensityListEntryPageNumber(page));
        };
        if free_slots & !0x0FFF != 0 {
            return Err(NdbError::InvalidDensityListEntryFreeSlots(free_slots));
        };

        Ok(Self(page | (free_slots as u32) << 20))
    }

    pub fn read(f: &mut dyn Read) -> io::Result<Self> {
        Ok(Self(f.read_u32::<LittleEndian>()?))
    }

    pub fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.0)
    }

    pub fn page(&self) -> u32 {
        self.0 & DENSITY_LIST_ENTRY_PAGE_NUMBER_MASK
    }

    pub fn free_slots(&self) -> u16 {
        (self.0 >> 20) as u16
    }
}

const DENSITY_LIST_FILE_OFFSET: u32 = 0x4200;

/// [DLISTPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/5d426b2d-ec10-4614-b768-46813652d5e3)
pub trait DensityListPage: Sized {
    type Trailer: PageTrailer;

    fn new(
        backfill_complete: bool,
        current_page: u32,
        entries: &[DensityListPageEntry],
        trailer: Self::Trailer,
    ) -> NdbResult<Self>;
    fn read<R: Read + Seek>(f: &mut R) -> io::Result<Self>;
    fn write<W: Write + Seek>(&self, f: &mut W) -> io::Result<()>;
    fn backfill_complete(&self) -> bool;
    fn current_page(&self) -> u32;
    fn entries(&self) -> &[DensityListPageEntry];
    fn trailer(&self) -> &Self::Trailer;
}

const MAX_UNICODE_DENSITY_LIST_ENTRY_COUNT: usize = 476 / mem::size_of::<DensityListPageEntry>();

pub struct UnicodeDensityListPage {
    backfill_complete: bool,
    current_page: u32,
    entry_count: u8,
    entries: [DensityListPageEntry; MAX_UNICODE_DENSITY_LIST_ENTRY_COUNT],
    trailer: UnicodePageTrailer,
}

impl DensityListPage for UnicodeDensityListPage {
    type Trailer = UnicodePageTrailer;

    fn new(
        backfill_complete: bool,
        current_page: u32,
        entries: &[DensityListPageEntry],
        trailer: UnicodePageTrailer,
    ) -> NdbResult<Self> {
        if entries.len() > MAX_UNICODE_DENSITY_LIST_ENTRY_COUNT {
            return Err(NdbError::InvalidDensityListEntryCount(entries.len()));
        }

        if trailer.page_type() != PageType::DensityList {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }

        let entry_count = entries.len() as u8;

        let mut buffer = [DensityListPageEntry(0); MAX_UNICODE_DENSITY_LIST_ENTRY_COUNT];
        buffer[..entries.len()].copy_from_slice(entries);
        let entries = buffer;

        Ok(Self {
            backfill_complete,
            current_page,
            entry_count,
            entries,
            trailer,
        })
    }

    fn read<R: Read + Seek>(f: &mut R) -> io::Result<Self> {
        f.seek(SeekFrom::Start(DENSITY_LIST_FILE_OFFSET as u64))?;

        let mut buffer = [0_u8; 496];
        f.read_exact(&mut buffer)?;
        let mut cursor = Cursor::new(buffer);

        // bFlags
        let backfill_complete = cursor.read_u8()? & 0x01 != 0;

        // cEntDList
        let entry_count = cursor.read_u8()?;
        if entry_count > MAX_UNICODE_DENSITY_LIST_ENTRY_COUNT as u8 {
            return Err(NdbError::InvalidDensityListEntryCount(entry_count as usize).into());
        }

        // wPadding
        if cursor.read_u16::<LittleEndian>()? != 0 {
            return Err(NdbError::InvalidDensityListPadding.into());
        }

        // ulCurrentPage
        let current_page = cursor.read_u32::<LittleEndian>()?;

        // rgDListPageEnt
        let mut entries = [DensityListPageEntry(0); MAX_UNICODE_DENSITY_LIST_ENTRY_COUNT];
        for entry in entries.iter_mut().take(entry_count as usize) {
            *entry = DensityListPageEntry::read(&mut cursor)?;
        }
        cursor.seek(SeekFrom::Current(
            ((MAX_UNICODE_DENSITY_LIST_ENTRY_COUNT - entry_count as usize)
                * mem::size_of::<DensityListPageEntry>()) as i64,
        ))?;

        // rgPadding
        let mut padding = [0_u8; 12];
        cursor.read_exact(&mut padding)?;
        if padding != [0; 12] {
            return Err(NdbError::InvalidDensityListPadding.into());
        }

        let buffer = cursor.into_inner();

        // pageTrailer
        let trailer = UnicodePageTrailer::read(f)?;
        if trailer.page_type() != PageType::DensityList {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()).into());
        }

        let crc = compute_crc(0, &buffer);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidPageCrc(crc).into());
        }

        Ok(Self {
            backfill_complete,
            current_page,
            entry_count,
            entries,
            trailer,
        })
    }

    fn write<W: Write + Seek>(&self, f: &mut W) -> io::Result<()> {
        let mut cursor = Cursor::new([0_u8; 496]);

        // bFlags
        cursor.write_u8(if self.backfill_complete { 0x01 } else { 0 })?;

        // cEntDList
        cursor.write_u8(self.entry_count)?;

        // wPadding
        cursor.write_u16::<LittleEndian>(0)?;

        // ulCurrentPage
        cursor.write_u32::<LittleEndian>(self.current_page)?;

        // rgDListPageEnt
        for entry in self.entries.iter() {
            entry.write(&mut cursor)?;
        }

        // rgPadding
        cursor.write_all(&[0; 12])?;

        let buffer = cursor.into_inner();
        let crc = compute_crc(0, &buffer);

        f.seek(SeekFrom::Start(DENSITY_LIST_FILE_OFFSET as u64))?;
        f.write_all(&buffer)?;

        // pageTrailer
        let trailer = UnicodePageTrailer {
            crc,
            ..self.trailer
        };
        trailer.write(f)
    }

    fn backfill_complete(&self) -> bool {
        self.backfill_complete
    }

    fn current_page(&self) -> u32 {
        self.current_page
    }

    fn entries(&self) -> &[DensityListPageEntry] {
        &self.entries[..self.entry_count as usize]
    }

    fn trailer(&self) -> &UnicodePageTrailer {
        &self.trailer
    }
}

const MAX_ANSI_DENSITY_LIST_ENTRY_COUNT: usize = 480 / mem::size_of::<DensityListPageEntry>();

pub struct AnsiDensityListPage {
    backfill_complete: bool,
    current_page: u32,
    entry_count: u8,
    entries: [DensityListPageEntry; MAX_ANSI_DENSITY_LIST_ENTRY_COUNT],
    trailer: AnsiPageTrailer,
}

impl DensityListPage for AnsiDensityListPage {
    type Trailer = AnsiPageTrailer;

    fn new(
        backfill_complete: bool,
        current_page: u32,
        entries: &[DensityListPageEntry],
        trailer: AnsiPageTrailer,
    ) -> NdbResult<Self> {
        if entries.len() > MAX_ANSI_DENSITY_LIST_ENTRY_COUNT {
            return Err(NdbError::InvalidDensityListEntryCount(entries.len()));
        }

        if trailer.page_type() != PageType::DensityList {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }

        let entry_count = entries.len() as u8;

        let mut buffer = [DensityListPageEntry(0); MAX_ANSI_DENSITY_LIST_ENTRY_COUNT];
        buffer[..entries.len()].copy_from_slice(entries);
        let entries = buffer;

        Ok(Self {
            backfill_complete,
            current_page,
            entry_count,
            entries,
            trailer,
        })
    }

    fn read<R: Read + Seek>(f: &mut R) -> io::Result<Self> {
        f.seek(SeekFrom::Start(DENSITY_LIST_FILE_OFFSET as u64))?;

        let mut buffer = [0_u8; 500];
        f.read_exact(&mut buffer)?;
        let mut cursor = Cursor::new(buffer);

        // bFlags
        let backfill_complete = cursor.read_u8()? & 0x01 != 0;

        // cEntDList
        let entry_count = cursor.read_u8()?;
        if entry_count > MAX_ANSI_DENSITY_LIST_ENTRY_COUNT as u8 {
            return Err(NdbError::InvalidDensityListEntryCount(entry_count as usize).into());
        }

        // wPadding
        if cursor.read_u16::<LittleEndian>()? != 0 {
            return Err(NdbError::InvalidDensityListPadding.into());
        }

        // ulCurrentPage
        let current_page = cursor.read_u32::<LittleEndian>()?;

        // rgDListPageEnt
        let mut entries = [DensityListPageEntry(0); MAX_ANSI_DENSITY_LIST_ENTRY_COUNT];
        for entry in entries.iter_mut().take(entry_count as usize) {
            *entry = DensityListPageEntry::read(&mut cursor)?;
        }
        cursor.seek(SeekFrom::Current(
            ((MAX_ANSI_DENSITY_LIST_ENTRY_COUNT - entry_count as usize)
                * mem::size_of::<DensityListPageEntry>()) as i64,
        ))?;

        // rgPadding
        let mut padding = [0_u8; 12];
        cursor.read_exact(&mut padding)?;
        if padding != [0; 12] {
            return Err(NdbError::InvalidDensityListPadding.into());
        }

        let buffer = cursor.into_inner();

        // pageTrailer
        let trailer = AnsiPageTrailer::read(f)?;
        if trailer.page_type() != PageType::DensityList {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()).into());
        }

        let crc = compute_crc(0, &buffer);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidPageCrc(crc).into());
        }

        Ok(Self {
            backfill_complete,
            current_page,
            entry_count,
            entries,
            trailer,
        })
    }

    fn write<W: Write + Seek>(&self, f: &mut W) -> io::Result<()> {
        let mut cursor = Cursor::new([0_u8; 500]);

        // bFlags
        cursor.write_u8(if self.backfill_complete { 0x01 } else { 0 })?;

        // cEntDList
        cursor.write_u8(self.entry_count)?;

        // wPadding
        cursor.write_u16::<LittleEndian>(0)?;

        // ulCurrentPage
        cursor.write_u32::<LittleEndian>(self.current_page)?;

        // rgDListPageEnt
        for entry in self.entries.iter() {
            entry.write(&mut cursor)?;
        }

        // rgPadding
        cursor.write_all(&[0; 12])?;

        let buffer = cursor.into_inner();
        let crc = compute_crc(0, &buffer);

        f.seek(SeekFrom::Start(DENSITY_LIST_FILE_OFFSET as u64))?;
        f.write_all(&buffer)?;

        // pageTrailer
        let trailer = AnsiPageTrailer {
            crc,
            ..self.trailer
        };
        trailer.write(f)
    }

    fn backfill_complete(&self) -> bool {
        self.backfill_complete
    }

    fn current_page(&self) -> u32 {
        self.current_page
    }

    fn entries(&self) -> &[DensityListPageEntry] {
        &self.entries[..self.entry_count as usize]
    }

    fn trailer(&self) -> &AnsiPageTrailer {
        &self.trailer
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

    #[test]
    fn test_magic_values() {
        assert_eq!(HEADER_MAGIC, 0x4E444221);
        assert_eq!(HEADER_MAGIC_CLIENT, 0x4D53);
    }
}
