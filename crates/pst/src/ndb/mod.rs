//! ## [Node Database (NDB) Layer](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/e4efaad0-1876-446e-9d34-bb921588f924)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io;
use thiserror::Error;

pub mod block;
pub mod block_id;
pub mod block_ref;
pub mod byte_index;
pub mod header;
pub mod node_id;
pub mod page;
pub mod root;

pub(crate) mod read_write;

use header::NdbCryptMethod;
use node_id::NodeId;
use page::PageType;

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
    #[error("Invalid DLISTPAGEENT dwPageNum: 0x{0:X}")]
    InvalidDensityListEntryPageNumber(u32),
    #[error("Invalid DLISTPAGEENT dwFreeSlots: 0x{0:04X}")]
    InvalidDensityListEntryFreeSlots(u16),
    #[error("Invalid DLISTPAGE cbEntDList: 0x{0:X}")]
    InvalidDensityListEntryCount(usize),
    #[error("Invalid DLISTPAGE rgPadding")]
    InvalidDensityListPadding,
    #[error("Invalid BTPAGE cLevel: 0x{0:02X}")]
    InvalidBTreePageLevel(u8),
    #[error("Invalid BTPAGE cEnt: {0}")]
    InvalidBTreeEntryCount(usize),
    #[error("Invalid BTPAGE cEntMax: {0}")]
    InvalidBTreeEntryMaxCount(u8),
    #[error("Invalid BTPAGE cbEnt: {0}")]
    InvalidBTreeEntrySize(u8),
    #[error("Invalid BTPAGE dwPadding: 0x{0:08X}")]
    InvalidBTreePagePadding(u32),
    #[error("BTENTRY not found: 0x{0:X}")]
    BTreePageNotFound(u64),
    #[error("Invalid NBTENTRY nid: 0x{0:X}")]
    InvalidNodeBTreeEntryNodeId(u64),
    #[error("Invalid BLOCKTRAILER cb: 0x{0:X}")]
    InvalidBlockSize(u16),
    #[error("Invalid BLOCKTRAILER dwCRC: 0x{0:08X}")]
    InvalidBlockCrc(u32),
    #[error("Invalid BLOCKTRAILER bid: 0x{0:X}")]
    InvalidUnicodeBlockTrailerId(u64),
    #[error("Invalid BLOCKTRAILER bid: 0x{0:X}")]
    InvalidAnsiBlockTrailerId(u32),
    #[error("Invalid internal block encoding: {0:?}")]
    InvalidInternalBlockEncoding(NdbCryptMethod),
    #[error("Invalid internal block data: {0:?}")]
    InvalidInternalBlockData(io::Error),
    #[error("Invalid internal block btype: 0x{0:02X}")]
    InvalidInternalBlockType(u8),
    #[error("Invalid internal block cLevel: 0x{0:02X}")]
    InvalidInternalBlockLevel(u8),
    #[error("Invalid internal block cEnt: 0x{0:X}")]
    InvalidInternalBlockEntryCount(u16),
    #[error("Invalid sub-node tree block dwPadding: 0x{0:08X}")]
    InvalidSubNodeBlockPadding(u32),
    #[error("Sub-node not found: {0:?}")]
    SubNodeNotFound(NodeId),
}

impl From<NdbError> for io::Error {
    fn from(err: NdbError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

pub type NdbResult<T> = Result<T, NdbError>;
