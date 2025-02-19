//! ## [Node Database (NDB) Layer](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/e4efaad0-1876-446e-9d34-bb921588f924)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use thiserror::Error;

use crate::crc::compute_crc;

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
    #[error("Invalid NBTENTRY nid: 0x{0:016X}")]
    InvalidNodeBTreeEntryNodeId(u64),
}

impl From<NdbError> for io::Error {
    fn from(err: NdbError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

pub type NdbResult<T> = Result<T, NdbError>;

pub mod block_id;
pub mod block_ref;
pub mod byte_index;
pub mod node_id;
pub mod page;

use block_id::*;
use block_ref::*;
use byte_index::*;
use node_id::*;
use page::*;

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
            next_page: Default::default(),
            unique: 0,
            nids: NDB_DEFAULT_NIDS,
            root,
            crypt_method,
            next_block: Default::default(),
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
            next_block: Default::default(),
            next_page: Default::default(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_values() {
        assert_eq!(HEADER_MAGIC, 0x4E444221);
        assert_eq!(HEADER_MAGIC_CLIENT, 0x4D53);
    }
}
