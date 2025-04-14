//! [HEADER](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/c9876f5a-664b-46a3-9887-ba63f113abf5)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

use super::{block_id::*, read_write::*, root::*, *};
use crate::{crc::compute_crc, AnsiPstFile, PstFile, UnicodePstFile};

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
    (0x400 << 5) | 0x01,
    (0x400 << 5) | 0x02,
    (0x4000 << 5) | 0x03,
    (0x10000 << 5) | 0x04,
    (0x400 << 5) | 0x05,
    (0x400 << 5) | 0x06,
    (0x400 << 5) | 0x07,
    (0x8000 << 5) | 0x08,
    (0x400 << 5) | 0x09,
    (0x400 << 5) | 0x0A,
    (0x400 << 5) | 0x0B,
    (0x400 << 5) | 0x0C,
    (0x400 << 5) | 0x0D,
    (0x400 << 5) | 0x0E,
    (0x400 << 5) | 0x0F,
    (0x400 << 5) | 0x10,
    (0x400 << 5) | 0x11,
    (0x400 << 5) | 0x12,
    (0x400 << 5) | 0x13,
    (0x400 << 5) | 0x14,
    (0x400 << 5) | 0x15,
    (0x400 << 5) | 0x16,
    (0x400 << 5) | 0x17,
    (0x400 << 5) | 0x18,
    (0x400 << 5) | 0x19,
    (0x400 << 5) | 0x1A,
    (0x400 << 5) | 0x1B,
    (0x400 << 5) | 0x1C,
    (0x400 << 5) | 0x1D,
    (0x400 << 5) | 0x1E,
    (0x400 << 5) | 0x1F,
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

pub trait Header<Pst>: Clone
where
    Pst: PstFile,
{
    fn version(&self) -> NdbVersion;
    fn crypt_method(&self) -> NdbCryptMethod;
    fn next_block(&self) -> <Pst as PstFile>::BlockId;
    fn next_page(&self) -> <Pst as PstFile>::BlockId;
    fn root(&self) -> &<Pst as PstFile>::Root;
    fn root_mut(&mut self) -> &mut <Pst as PstFile>::Root;
}

#[derive(Clone, Debug)]
pub struct UnicodeHeader {
    next_page: UnicodeBlockId,
    unique: u32,
    nids: [u32; 32],
    root: UnicodeRoot,
    free_map: [u8; 128],
    free_page_map: [u8; 128],
    crypt_method: NdbCryptMethod,
    next_block: UnicodeBlockId,

    reserved1: u32,
    reserved2: u32,
    unused1: u64,
    unused2: u64,
    reserved3: [u8; 36],
}

impl UnicodeHeader {
    pub fn new(root: UnicodeRoot, crypt_method: NdbCryptMethod) -> Self {
        Self {
            next_page: Default::default(),
            unique: 0,
            nids: NDB_DEFAULT_NIDS,
            root,
            free_map: [0xFF; 128],
            free_page_map: [0xFF; 128],
            crypt_method,
            next_block: Default::default(),
            reserved1: 0,
            reserved2: 0,
            unused1: 0,
            unused2: 0,
            reserved3: [0; 36],
        }
    }
}

impl Header<UnicodePstFile> for UnicodeHeader {
    fn version(&self) -> NdbVersion {
        NdbVersion::Unicode
    }

    fn crypt_method(&self) -> NdbCryptMethod {
        self.crypt_method
    }

    fn next_block(&self) -> <UnicodePstFile as PstFile>::BlockId {
        self.next_block
    }

    fn next_page(&self) -> <UnicodePstFile as PstFile>::BlockId {
        self.next_page
    }

    fn root(&self) -> &<UnicodePstFile as PstFile>::Root {
        &self.root
    }

    fn root_mut(&mut self) -> &mut <UnicodePstFile as PstFile>::Root {
        &mut self.root
    }
}

impl HeaderReadWrite<UnicodePstFile> for UnicodeHeader {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
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
        let unused1 = cursor.read_u64::<LittleEndian>()?;

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
        let unused2 = cursor.read_u64::<LittleEndian>()?;

        // root
        let root = UnicodeRoot::read(&mut cursor)?;

        // dwAlign
        let align = cursor.read_u32::<LittleEndian>()?;
        if align != 0 {
            return Err(NdbError::InvalidNdbHeaderAlignValue(align).into());
        }

        // rgbFM
        let mut free_map = [0; 128];
        cursor.read_exact(&mut free_map)?;

        // rgbFP
        let mut free_page_map = [0; 128];
        cursor.read_exact(&mut free_page_map)?;

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
            free_map,
            free_page_map,
            crypt_method,
            next_block,
            reserved1,
            reserved2,
            unused1,
            unused2,
            reserved3,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
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
        cursor.write_u64::<LittleEndian>(self.unused1)?;
        // bidNextP
        self.next_page.write(&mut cursor)?;
        // dwUnique
        cursor.write_u32::<LittleEndian>(self.unique)?;
        // rgnid
        for nid in self.nids.iter() {
            cursor.write_u32::<LittleEndian>(*nid)?;
        }
        // qwUnused
        cursor.write_u64::<LittleEndian>(self.unused2)?;
        // root
        self.root.write(&mut cursor)?;
        // dwAlign
        cursor.write_u32::<LittleEndian>(0)?;
        // rgbFM
        cursor.write_all(&self.free_map)?;
        // rgbFP
        cursor.write_all(&self.free_page_map)?;
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

    fn update_unique(&mut self) {
        self.unique = self.unique.wrapping_add(1);
    }

    fn first_free_map(&mut self) -> &mut [u8] {
        &mut self.free_map
    }
}

#[derive(Clone, Debug)]
pub struct AnsiHeader {
    next_block: AnsiBlockId,
    next_page: AnsiBlockId,
    unique: u32,
    nids: [u32; 32],
    root: AnsiRoot,
    free_map: [u8; 128],
    free_page_map: [u8; 128],
    crypt_method: NdbCryptMethod,

    reserved1: u32,
    reserved2: u32,
    reserved3: [u8; 36],
}

impl AnsiHeader {
    pub fn new(root: AnsiRoot, crypt_method: NdbCryptMethod) -> Self {
        Self {
            next_block: Default::default(),
            next_page: Default::default(),
            unique: 0,
            nids: NDB_DEFAULT_NIDS,
            root,
            free_map: [0xFF; 128],
            free_page_map: [0xFF; 128],
            crypt_method,
            reserved1: 0,
            reserved2: 0,
            reserved3: [0; 36],
        }
    }
}

impl Header<AnsiPstFile> for AnsiHeader {
    fn version(&self) -> NdbVersion {
        NdbVersion::Ansi
    }

    fn crypt_method(&self) -> NdbCryptMethod {
        self.crypt_method
    }

    fn next_block(&self) -> <AnsiPstFile as PstFile>::BlockId {
        self.next_block
    }

    fn next_page(&self) -> <AnsiPstFile as PstFile>::BlockId {
        self.next_page
    }

    fn root(&self) -> &<AnsiPstFile as PstFile>::Root {
        &self.root
    }

    fn root_mut(&mut self) -> &mut <AnsiPstFile as PstFile>::Root {
        &mut self.root
    }
}

impl HeaderReadWrite<AnsiPstFile> for AnsiHeader {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
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
        let mut free_map = [0; 128];
        cursor.read_exact(&mut free_map)?;

        // rgbFP
        let mut free_page_map = [0; 128];
        cursor.read_exact(&mut free_page_map)?;

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

        // ullReserved, dwReserved (total 12 bytes)
        let mut reserved = [0_u8; 12];
        cursor.read_exact(&mut reserved)?;
        if reserved != [0; 12] {
            return Err(NdbError::InvalidNdbHeaderAnsiReservedBytes.into());
        }

        // rgbReserved2, bReserved, rgbReserved3 (total 36 bytes)
        let mut reserved3 = [0_u8; 36];
        cursor.read_exact(&mut reserved3)?;

        Ok(Self {
            next_page,
            unique,
            nids,
            root,
            free_map,
            free_page_map,
            crypt_method,
            next_block,
            reserved1,
            reserved2,
            reserved3,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
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
        cursor.write_all(&self.free_map)?;
        // rgbFP
        cursor.write_all(&self.free_page_map)?;
        // bSentinel
        cursor.write_u8(NDB_SENTINEL)?;
        // bCryptMethod
        cursor.write_u8(self.crypt_method as u8)?;
        // rgbReserved
        cursor.write_u16::<LittleEndian>(0)?;
        // ullReserved, dwReserved (total 12 bytes)
        cursor.write_all(&[0_u8; 12])?;
        // rgbReserved2, bReserved, rgbReserved3 (total 36 bytes)
        cursor.write_all(&self.reserved3)?;

        let crc_data = cursor.into_inner();
        let crc_partial = compute_crc(0, &crc_data[..471]);

        // dwMagic
        f.write_u32::<LittleEndian>(HEADER_MAGIC)?;
        // dwCRCPartial
        f.write_u32::<LittleEndian>(crc_partial)?;

        f.write_all(&crc_data)
    }

    fn update_unique(&mut self) {
        self.unique = self.unique.wrapping_add(1);
    }

    fn first_free_map(&mut self) -> &mut [u8] {
        &mut self.free_map
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
