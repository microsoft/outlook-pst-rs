//! [Pages](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/5774b4f2-cdc4-453e-996a-8c8230116930)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use core::mem;
use std::{
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    marker::PhantomData,
};

use super::*;
use crate::{block_sig::compute_sig, crc::compute_crc};

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

pub trait BTreeEntry: Sized + Copy + Default {
    const ENTRY_SIZE: usize;

    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

/// [BTPAGE](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/4f0cd8e7-c2d0-4975-90a4-d417cfca77f8)
pub trait BTreePage: Sized {
    type Entry: BTreeEntry;
    type Trailer: PageTrailer;

    fn new(level: u8, entries: &[Self::Entry], trailer: Self::Trailer) -> NdbResult<Self>;
    fn level(&self) -> u8;
    fn entries(&self) -> &[Self::Entry];
    fn trailer(&self) -> &Self::Trailer;
}

const UNICODE_BTREE_ENTRIES_SIZE: usize = 488;

pub trait UnicodeBTreePage<Entry>: BTreePage<Entry = Entry, Trailer = UnicodePageTrailer>
where
    Entry: BTreeEntry,
{
    const MAX_BTREE_ENTRIES: usize = UNICODE_BTREE_ENTRIES_SIZE / Entry::ENTRY_SIZE;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let mut buffer = [0_u8; 496];
        f.read_exact(&mut buffer)?;
        let mut cursor = Cursor::new(buffer);

        cursor.seek(SeekFrom::Start(UNICODE_BTREE_ENTRIES_SIZE as u64))?;

        // cEnt
        let entry_count = usize::from(cursor.read_u8()?);
        if entry_count > Self::MAX_BTREE_ENTRIES {
            return Err(NdbError::InvalidBTreeEntryCount(entry_count).into());
        }

        // cEntMax
        let max_entries = cursor.read_u8()?;
        if usize::from(max_entries) != Self::MAX_BTREE_ENTRIES {
            return Err(NdbError::InvalidBTreeEntryMaxCount(max_entries).into());
        }

        // cbEnt
        let entry_size = cursor.read_u8()?;
        if usize::from(entry_size) != Entry::ENTRY_SIZE {
            return Err(NdbError::InvalidBTreeEntrySize(entry_size).into());
        }

        // cLevel
        let level = cursor.read_u8()?;
        if !(0..=8).contains(&level) {
            return Err(NdbError::InvalidBTreePageLevel(level).into());
        }

        // dwPadding
        let padding = cursor.read_u32::<LittleEndian>()?;
        if padding != 0 {
            return Err(NdbError::InvalidBTreePagePadding(padding).into());
        }

        // pageTrailer
        let trailer = UnicodePageTrailer::read(f)?;
        if trailer.page_type() != PageType::BlockBTree && trailer.page_type() != PageType::NodeBTree
        {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()).into());
        }

        let buffer = cursor.into_inner();
        let crc = compute_crc(0, &buffer);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidPageCrc(crc).into());
        }

        // rgentries
        let mut cursor = Cursor::new(buffer);
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            entries.push(<Self::Entry as BTreeEntry>::read(&mut cursor)?);
        }

        Ok(<Self as BTreePage>::new(level, &entries, trailer)?)
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        let mut cursor = Cursor::new([0_u8; 496]);

        // rgentries
        let entries = self.entries();
        for entry in entries.iter().take(Self::MAX_BTREE_ENTRIES) {
            <Self::Entry as BTreeEntry>::write(entry, &mut cursor)?;
        }
        if entries.len() < Self::MAX_BTREE_ENTRIES {
            let entry = Default::default();
            for _ in entries.len()..Self::MAX_BTREE_ENTRIES {
                <Self::Entry as BTreeEntry>::write(&entry, &mut cursor)?;
            }
        }

        // cEnt
        cursor.write_u8(entries.len() as u8)?;

        // cEntMax
        cursor.write_u8(Self::MAX_BTREE_ENTRIES as u8)?;

        // cbEnt
        cursor.write_u8(Entry::ENTRY_SIZE as u8)?;

        // cLevel
        cursor.write_u8(self.level())?;

        // dwPadding
        cursor.write_u32::<LittleEndian>(0)?;

        let buffer = cursor.into_inner();
        let crc = compute_crc(0, &buffer);

        f.write_all(&buffer)?;

        // pageTrailer
        let trailer = UnicodePageTrailer {
            crc,
            ..*self.trailer()
        };
        trailer.write(f)
    }
}

pub struct UnicodeBTreeEntryPage {
    level: u8,
    entry_count: u8,
    entries: [UnicodeBTreePageEntry;
        <Self as UnicodeBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES],
    trailer: UnicodePageTrailer,
}

impl BTreePage for UnicodeBTreeEntryPage {
    type Entry = UnicodeBTreePageEntry;
    type Trailer = UnicodePageTrailer;

    fn new(
        level: u8,
        entries: &[UnicodeBTreePageEntry],
        trailer: UnicodePageTrailer,
    ) -> NdbResult<Self> {
        if !(1..=8).contains(&level) {
            return Err(NdbError::InvalidBTreePageLevel(level));
        }

        if entries.len() > <Self as UnicodeBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES
        {
            return Err(NdbError::InvalidBTreeEntryCount(entries.len()));
        }

        if trailer.page_type() != PageType::BlockBTree && trailer.page_type() != PageType::NodeBTree
        {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }

        let mut buffer = [UnicodeBTreePageEntry::default();
            <Self as UnicodeBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES];
        buffer[..entries.len()].copy_from_slice(entries);

        Ok(Self {
            level,
            entry_count: entries.len() as u8,
            entries: buffer,
            trailer,
        })
    }

    fn level(&self) -> u8 {
        self.level
    }

    fn entries(&self) -> &[UnicodeBTreePageEntry] {
        &self.entries[..self.entry_count as usize]
    }

    fn trailer(&self) -> &UnicodePageTrailer {
        &self.trailer
    }
}

impl UnicodeBTreePage<UnicodeBTreePageEntry> for UnicodeBTreeEntryPage {}

const ANSI_BTREE_ENTRIES_SIZE: usize = 496;

pub trait AnsiBTreePage<Entry>: BTreePage<Entry = Entry, Trailer = AnsiPageTrailer>
where
    Entry: BTreeEntry,
{
    const MAX_BTREE_ENTRIES: usize = ANSI_BTREE_ENTRIES_SIZE / Entry::ENTRY_SIZE;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let mut buffer = [0_u8; 500];
        f.read_exact(&mut buffer)?;
        let mut cursor = Cursor::new(buffer);

        cursor.seek(SeekFrom::Start(ANSI_BTREE_ENTRIES_SIZE as u64))?;

        // cEnt
        let entry_count = usize::from(cursor.read_u8()?);
        if entry_count > Self::MAX_BTREE_ENTRIES {
            return Err(NdbError::InvalidBTreeEntryCount(entry_count).into());
        }

        // cEntMax
        let max_entries = cursor.read_u8()?;
        if usize::from(max_entries) != Self::MAX_BTREE_ENTRIES {
            return Err(NdbError::InvalidBTreeEntryMaxCount(max_entries).into());
        }

        // cbEnt
        let entry_size = cursor.read_u8()?;
        if usize::from(entry_size) != Entry::ENTRY_SIZE {
            return Err(NdbError::InvalidBTreeEntrySize(entry_size).into());
        }

        // cLevel
        let level = cursor.read_u8()?;
        if !(0..=8).contains(&level) {
            return Err(NdbError::InvalidBTreePageLevel(level).into());
        }

        // pageTrailer
        let trailer = AnsiPageTrailer::read(f)?;
        if trailer.page_type() != PageType::BlockBTree && trailer.page_type() != PageType::NodeBTree
        {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()).into());
        }

        let buffer = cursor.into_inner();
        let crc = compute_crc(0, &buffer);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidPageCrc(crc).into());
        }

        // rgentries
        let mut cursor = Cursor::new(buffer);
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            entries.push(<Self::Entry as BTreeEntry>::read(&mut cursor)?);
        }

        Ok(<Self as BTreePage>::new(level, &entries, trailer)?)
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        let mut cursor = Cursor::new([0_u8; 500]);

        // rgentries
        let entries = self.entries();
        for entry in entries.iter().take(Self::MAX_BTREE_ENTRIES) {
            <Self::Entry as BTreeEntry>::write(entry, &mut cursor)?;
        }
        if entries.len() < Self::MAX_BTREE_ENTRIES {
            let entry = Default::default();
            for _ in entries.len()..Self::MAX_BTREE_ENTRIES {
                <Self::Entry as BTreeEntry>::write(&entry, &mut cursor)?;
            }
        }

        // cEnt
        cursor.write_u8(entries.len() as u8)?;

        // cEntMax
        cursor.write_u8(Self::MAX_BTREE_ENTRIES as u8)?;

        // cbEnt
        cursor.write_u8(Entry::ENTRY_SIZE as u8)?;

        // cLevel
        cursor.write_u8(self.level())?;

        let buffer = cursor.into_inner();
        let crc = compute_crc(0, &buffer);

        f.write_all(&buffer)?;

        // pageTrailer
        let trailer = AnsiPageTrailer {
            crc,
            ..*self.trailer()
        };
        trailer.write(f)
    }
}

pub struct AnsiBTreeEntryPage {
    level: u8,
    entry_count: u8,
    entries: [AnsiBTreePageEntry;
        <Self as AnsiBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES],
    trailer: AnsiPageTrailer,
}

impl BTreePage for AnsiBTreeEntryPage {
    type Entry = AnsiBTreePageEntry;
    type Trailer = AnsiPageTrailer;

    fn new(level: u8, entries: &[AnsiBTreePageEntry], trailer: AnsiPageTrailer) -> NdbResult<Self> {
        if !(1..=8).contains(&level) {
            return Err(NdbError::InvalidBTreePageLevel(level));
        }

        if entries.len() > <Self as AnsiBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES {
            return Err(NdbError::InvalidBTreeEntryCount(entries.len()));
        }

        if trailer.page_type() != PageType::BlockBTree && trailer.page_type() != PageType::NodeBTree
        {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }

        let mut buffer = [AnsiBTreePageEntry::default();
            <Self as AnsiBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES];
        buffer[..entries.len()].copy_from_slice(entries);

        Ok(Self {
            level,
            entry_count: entries.len() as u8,
            entries: buffer,
            trailer,
        })
    }

    fn level(&self) -> u8 {
        self.level
    }

    fn entries(&self) -> &[AnsiBTreePageEntry] {
        &self.entries[..self.entry_count as usize]
    }

    fn trailer(&self) -> &AnsiPageTrailer {
        &self.trailer
    }
}

impl AnsiBTreePage<AnsiBTreePageEntry> for AnsiBTreeEntryPage {}

/// [BTENTRY](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/bc8052a3-f300-4022-be31-f0f408fffca0)
pub trait BTreePageEntry: BTreeEntry {
    type Key: BlockId;
    type Block: BlockRef;
    const ENTRY_SIZE: usize;

    fn new(key: Self::Key, block: Self::Block) -> Self;
    fn extend_key(node: NodeId) -> Self::Key;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        Ok(Self::new(Self::Key::read(f)?, Self::Block::read(f)?))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.key().write(f)?;
        self.block().write(f)
    }

    fn key(&self) -> Self::Key;
    fn block(&self) -> Self::Block;
}

impl<Entry> BTreeEntry for Entry
where
    Entry: BTreePageEntry,
{
    const ENTRY_SIZE: usize = <Entry as BTreePageEntry>::ENTRY_SIZE;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        Ok(Self::new(
            <Self as BTreePageEntry>::Key::read(f)?,
            <Self as BTreePageEntry>::Block::read(f)?,
        ))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.key().write(f)?;
        self.block().write(f)
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct UnicodeBTreePageEntry {
    key: UnicodeBlockId,
    block: UnicodeBlockRef,
}

impl BTreePageEntry for UnicodeBTreePageEntry {
    type Key = UnicodeBlockId;
    type Block = UnicodeBlockRef;
    const ENTRY_SIZE: usize = 24;

    fn new(key: UnicodeBlockId, block: UnicodeBlockRef) -> Self {
        Self { key, block }
    }

    fn extend_key(node: NodeId) -> Self::Key {
        UnicodeBlockId::from(u64::from(u32::from(node)))
    }

    fn key(&self) -> UnicodeBlockId {
        self.key
    }

    fn block(&self) -> UnicodeBlockRef {
        self.block
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct AnsiBTreePageEntry {
    key: AnsiBlockId,
    block: AnsiBlockRef,
}

impl BTreePageEntry for AnsiBTreePageEntry {
    type Key = AnsiBlockId;
    type Block = AnsiBlockRef;
    const ENTRY_SIZE: usize = 12;

    fn new(key: AnsiBlockId, block: AnsiBlockRef) -> Self {
        Self { key, block }
    }

    fn extend_key(node: NodeId) -> Self::Key {
        AnsiBlockId::from(u32::from(node))
    }

    fn key(&self) -> AnsiBlockId {
        self.key
    }

    fn block(&self) -> AnsiBlockRef {
        self.block
    }
}

/// [BBTENTRY](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/53a4b926-8ac4-45c9-9c6d-8358d951dbcd)
pub trait BlockBTreeEntry: BTreeEntry {
    type Block: BlockRef;

    fn new(block: Self::Block, size: u16) -> Self;
    fn block(&self) -> Self::Block;
    fn size(&self) -> u16;
    fn ref_count(&self) -> u16;
}

#[derive(Copy, Clone, Default, Debug)]
pub struct UnicodeBlockBTreeEntry {
    block: UnicodeBlockRef,
    size: u16,
    ref_count: u16,
    padding: u32,
}

impl BTreeEntry for UnicodeBlockBTreeEntry {
    const ENTRY_SIZE: usize = 24;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let block = UnicodeBlockRef::read(f)?;
        let size = f.read_u16::<LittleEndian>()?;
        let ref_count = f.read_u16::<LittleEndian>()?;
        let padding = f.read_u32::<LittleEndian>()?;

        Ok(Self {
            block,
            size,
            ref_count,
            padding,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.block.write(f)?;
        f.write_u16::<LittleEndian>(self.size)?;
        f.write_u16::<LittleEndian>(self.ref_count)?;
        f.write_u32::<LittleEndian>(self.padding)
    }
}

impl BlockBTreeEntry for UnicodeBlockBTreeEntry {
    type Block = UnicodeBlockRef;

    fn new(block: Self::Block, size: u16) -> Self {
        Self {
            block,
            size,
            ref_count: 1,
            ..Default::default()
        }
    }

    fn block(&self) -> UnicodeBlockRef {
        self.block
    }

    fn size(&self) -> u16 {
        self.size
    }

    fn ref_count(&self) -> u16 {
        self.ref_count
    }
}

pub struct UnicodeBlockBTreePage {
    entry_count: u8,
    entries: [UnicodeBlockBTreeEntry;
        <Self as UnicodeBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES],
    trailer: UnicodePageTrailer,
}

impl BTreePage for UnicodeBlockBTreePage {
    type Entry = UnicodeBlockBTreeEntry;
    type Trailer = UnicodePageTrailer;

    fn new(
        level: u8,
        entries: &[UnicodeBlockBTreeEntry],
        trailer: UnicodePageTrailer,
    ) -> NdbResult<Self> {
        if level != 0 {
            return Err(NdbError::InvalidBTreePageLevel(level));
        }

        if entries.len() > <Self as UnicodeBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES
        {
            return Err(NdbError::InvalidBTreeEntryCount(entries.len()));
        }

        if trailer.page_type() != PageType::BlockBTree {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }

        let mut buffer = [UnicodeBlockBTreeEntry::default();
            <Self as UnicodeBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES];
        buffer[..entries.len()].copy_from_slice(entries);

        Ok(Self {
            entry_count: entries.len() as u8,
            entries: buffer,
            trailer,
        })
    }

    fn level(&self) -> u8 {
        0
    }

    fn entries(&self) -> &[UnicodeBlockBTreeEntry] {
        &self.entries[..self.entry_count as usize]
    }

    fn trailer(&self) -> &UnicodePageTrailer {
        &self.trailer
    }
}

impl UnicodeBTreePage<UnicodeBlockBTreeEntry> for UnicodeBlockBTreePage {}

#[derive(Copy, Clone, Default, Debug)]
pub struct AnsiBlockBTreeEntry {
    block: AnsiBlockRef,
    size: u16,
    ref_count: u16,
}

impl BTreeEntry for AnsiBlockBTreeEntry {
    const ENTRY_SIZE: usize = 12;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        Ok(Self {
            block: AnsiBlockRef::read(f)?,
            size: f.read_u16::<LittleEndian>()?,
            ref_count: f.read_u16::<LittleEndian>()?,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.block.write(f)?;
        f.write_u16::<LittleEndian>(self.size)?;
        f.write_u16::<LittleEndian>(self.ref_count)
    }
}

impl BlockBTreeEntry for AnsiBlockBTreeEntry {
    type Block = AnsiBlockRef;

    fn new(block: Self::Block, size: u16) -> Self {
        Self {
            block,
            size,
            ref_count: 1,
        }
    }

    fn block(&self) -> AnsiBlockRef {
        self.block
    }

    fn size(&self) -> u16 {
        self.size
    }

    fn ref_count(&self) -> u16 {
        self.ref_count
    }
}

pub struct AnsiBlockBTreePage {
    entry_count: u8,
    entries: [AnsiBlockBTreeEntry;
        <Self as AnsiBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES],
    trailer: AnsiPageTrailer,
}

impl BTreePage for AnsiBlockBTreePage {
    type Entry = AnsiBlockBTreeEntry;
    type Trailer = AnsiPageTrailer;

    fn new(
        level: u8,
        entries: &[AnsiBlockBTreeEntry],
        trailer: AnsiPageTrailer,
    ) -> NdbResult<Self> {
        if level != 0 {
            return Err(NdbError::InvalidBTreePageLevel(level));
        }

        if entries.len() > <Self as AnsiBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES {
            return Err(NdbError::InvalidBTreeEntryCount(entries.len()));
        }

        if trailer.page_type() != PageType::BlockBTree {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }

        let mut buffer = [AnsiBlockBTreeEntry::default();
            <Self as AnsiBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES];
        buffer[..entries.len()].copy_from_slice(entries);

        Ok(Self {
            entry_count: entries.len() as u8,
            entries: buffer,
            trailer,
        })
    }

    fn level(&self) -> u8 {
        0
    }

    fn entries(&self) -> &[AnsiBlockBTreeEntry] {
        &self.entries[..self.entry_count as usize]
    }

    fn trailer(&self) -> &AnsiPageTrailer {
        &self.trailer
    }
}

impl AnsiBTreePage<AnsiBlockBTreeEntry> for AnsiBlockBTreePage {}

/// [NBTENTRY](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/53a4b926-8ac4-45c9-9c6d-8358d951dbcd)
pub trait NodeBTreeEntry: BTreeEntry {
    type Block: BlockId;

    fn new(
        node: NodeId,
        data: Self::Block,
        sub_node: Option<Self::Block>,
        parent: Option<NodeId>,
    ) -> Self;
    fn node(&self) -> NodeId;
    fn data(&self) -> Self::Block;
    fn sub_node(&self) -> Option<Self::Block>;
    fn parent(&self) -> Option<NodeId>;
}

#[derive(Copy, Clone, Default, Debug)]
pub struct UnicodeNodeBTreeEntry {
    node: NodeId,
    data: UnicodeBlockId,
    sub_node: Option<UnicodeBlockId>,
    parent: Option<NodeId>,
    padding: u32,
}

impl BTreeEntry for UnicodeNodeBTreeEntry {
    const ENTRY_SIZE: usize = 32;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        // nid
        let node = f.read_u64::<LittleEndian>()?;
        let Ok(node) = u32::try_from(node) else {
            return Err(NdbError::InvalidNodeBTreeEntryNodeId(node).into());
        };
        let node = NodeId::from(node);

        // bidData
        let data = UnicodeBlockId::read(f)?;

        // bidSub
        let sub_node = UnicodeBlockId::read(f)?;
        let sub_node = if u64::from(sub_node) == 0 {
            None
        } else {
            Some(sub_node)
        };

        // nidParent
        let parent = NodeId::read(f)?;
        let parent = if u32::from(parent) == 0 {
            None
        } else {
            Some(parent)
        };

        // dwPadding
        let padding = f.read_u32::<LittleEndian>()?;

        Ok(Self {
            node,
            data,
            sub_node,
            parent,
            padding,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        // nid
        f.write_u64::<LittleEndian>(u64::from(u32::from(self.node)))?;

        // bidData
        self.data.write(f)?;

        // bidSub
        self.sub_node.unwrap_or_default().write(f)?;

        // nidParent
        self.parent.unwrap_or_default().write(f)?;

        // dwPadding
        f.write_u32::<LittleEndian>(self.padding)
    }
}

impl NodeBTreeEntry for UnicodeNodeBTreeEntry {
    type Block = UnicodeBlockId;

    fn new(
        node: NodeId,
        data: Self::Block,
        sub_node: Option<Self::Block>,
        parent: Option<NodeId>,
    ) -> Self {
        Self {
            node,
            data,
            sub_node,
            parent,
            ..Default::default()
        }
    }

    fn node(&self) -> NodeId {
        self.node
    }

    fn data(&self) -> UnicodeBlockId {
        self.data
    }

    fn sub_node(&self) -> Option<UnicodeBlockId> {
        self.sub_node
    }

    fn parent(&self) -> Option<NodeId> {
        self.parent
    }
}

pub struct UnicodeNodeBTreePage {
    entry_count: u8,
    entries: [UnicodeNodeBTreeEntry;
        <Self as UnicodeBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES],
    trailer: UnicodePageTrailer,
}

impl BTreePage for UnicodeNodeBTreePage {
    type Entry = UnicodeNodeBTreeEntry;
    type Trailer = UnicodePageTrailer;

    fn new(
        level: u8,
        entries: &[UnicodeNodeBTreeEntry],
        trailer: UnicodePageTrailer,
    ) -> NdbResult<Self> {
        if level != 0 {
            return Err(NdbError::InvalidBTreePageLevel(level));
        }

        if entries.len() > <Self as UnicodeBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES
        {
            return Err(NdbError::InvalidBTreeEntryCount(entries.len()));
        }

        if trailer.page_type() != PageType::NodeBTree {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }

        let mut buffer = [UnicodeNodeBTreeEntry::default();
            <Self as UnicodeBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES];
        buffer[..entries.len()].copy_from_slice(entries);

        Ok(Self {
            entry_count: entries.len() as u8,
            entries: buffer,
            trailer,
        })
    }

    fn level(&self) -> u8 {
        0
    }

    fn entries(&self) -> &[UnicodeNodeBTreeEntry] {
        &self.entries[..self.entry_count as usize]
    }

    fn trailer(&self) -> &UnicodePageTrailer {
        &self.trailer
    }
}

impl UnicodeBTreePage<UnicodeNodeBTreeEntry> for UnicodeNodeBTreePage {}

#[derive(Copy, Clone, Default, Debug)]
pub struct AnsiNodeBTreeEntry {
    node: NodeId,
    data: AnsiBlockId,
    sub_node: Option<AnsiBlockId>,
    parent: Option<NodeId>,
}

impl BTreeEntry for AnsiNodeBTreeEntry {
    const ENTRY_SIZE: usize = 16;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        // nid
        let node = NodeId::read(f)?;

        // bidData
        let data = AnsiBlockId::read(f)?;

        // bidSub
        let sub_node = AnsiBlockId::read(f)?;
        let sub_node = if u32::from(sub_node) == 0 {
            None
        } else {
            Some(sub_node)
        };

        // nidParent
        let parent = NodeId::read(f)?;
        let parent = if u32::from(parent) == 0 {
            None
        } else {
            Some(parent)
        };

        Ok(Self {
            node,
            data,
            sub_node,
            parent,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        // nid
        self.node.write(f)?;

        // bidData
        self.data.write(f)?;

        // bidSub
        self.sub_node.unwrap_or_default().write(f)?;

        // nidParent
        self.parent.unwrap_or_default().write(f)
    }
}

impl NodeBTreeEntry for AnsiNodeBTreeEntry {
    type Block = AnsiBlockId;

    fn new(
        node: NodeId,
        data: Self::Block,
        sub_node: Option<Self::Block>,
        parent: Option<NodeId>,
    ) -> Self {
        Self {
            node,
            data,
            sub_node,
            parent,
        }
    }

    fn node(&self) -> NodeId {
        self.node
    }

    fn data(&self) -> AnsiBlockId {
        self.data
    }

    fn sub_node(&self) -> Option<AnsiBlockId> {
        self.sub_node
    }

    fn parent(&self) -> Option<NodeId> {
        self.parent
    }
}

pub struct AnsiNodeBTreePage {
    entry_count: u8,
    entries: [AnsiNodeBTreeEntry;
        <Self as AnsiBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES],
    trailer: AnsiPageTrailer,
}

impl BTreePage for AnsiNodeBTreePage {
    type Entry = AnsiNodeBTreeEntry;
    type Trailer = AnsiPageTrailer;

    fn new(level: u8, entries: &[AnsiNodeBTreeEntry], trailer: AnsiPageTrailer) -> NdbResult<Self> {
        if level != 0 {
            return Err(NdbError::InvalidBTreePageLevel(level));
        }

        if entries.len() > <Self as AnsiBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES {
            return Err(NdbError::InvalidBTreeEntryCount(entries.len()));
        }

        if trailer.page_type() != PageType::NodeBTree {
            return Err(NdbError::UnexpectedPageType(trailer.page_type()));
        }

        let mut buffer = [AnsiNodeBTreeEntry::default();
            <Self as AnsiBTreePage<<Self as BTreePage>::Entry>>::MAX_BTREE_ENTRIES];
        buffer[..entries.len()].copy_from_slice(entries);

        Ok(Self {
            entry_count: entries.len() as u8,
            entries: buffer,
            trailer,
        })
    }

    fn level(&self) -> u8 {
        0
    }

    fn entries(&self) -> &[AnsiNodeBTreeEntry] {
        &self.entries[..self.entry_count as usize]
    }

    fn trailer(&self) -> &AnsiPageTrailer {
        &self.trailer
    }
}

impl AnsiBTreePage<AnsiNodeBTreeEntry> for AnsiNodeBTreePage {}

pub enum UnicodeBTree<LeafPage, Entry>
where
    LeafPage: UnicodeBTreePage<Entry>,
    Entry: BTreeEntry,
{
    Intermediate(Box<UnicodeBTreeEntryPage>),
    Leaf(Box<LeafPage>, PhantomData<Entry>),
}

impl<LeafPage, Entry> UnicodeBTree<LeafPage, Entry>
where
    LeafPage: UnicodeBTreePage<Entry>,
    Entry: BTreeEntry,
{
    pub fn read<R: Read + Seek>(f: &mut R, block: UnicodeBlockRef) -> io::Result<Self> {
        f.seek(SeekFrom::Start(block.index().index()))?;

        let mut buffer = [0_u8; 512];
        f.read_exact(&mut buffer)?;
        let mut cursor = Cursor::new(buffer);

        cursor.seek(SeekFrom::Start(UNICODE_BTREE_ENTRIES_SIZE as u64 + 3))?;
        let level = cursor.read_u8()?;

        cursor.seek(SeekFrom::Start(0))?;
        Ok(if level == 0 {
            UnicodeBTree::Leaf(Box::new(LeafPage::read(&mut cursor)?), PhantomData)
        } else {
            UnicodeBTree::Intermediate(Box::new(UnicodeBTreeEntryPage::read(&mut cursor)?))
        })
    }

    pub fn write<W: Write + Seek>(&self, f: &mut W, block: UnicodeBlockRef) -> io::Result<()> {
        f.seek(SeekFrom::Start(block.index().index()))?;

        match self {
            UnicodeBTree::Intermediate(page) => page.write(f),
            UnicodeBTree::Leaf(page, _) => page.write(f),
        }
    }
}

pub type UnicodeBlockBTree = UnicodeBTree<UnicodeBlockBTreePage, UnicodeBlockBTreeEntry>;
pub type UnicodeNodeBTree = UnicodeBTree<UnicodeNodeBTreePage, UnicodeNodeBTreeEntry>;

pub enum AnsiBTree<LeafPage, Entry>
where
    LeafPage: AnsiBTreePage<Entry>,
    Entry: BTreeEntry,
{
    Intermediate(Box<AnsiBTreeEntryPage>),
    Leaf(Box<LeafPage>, PhantomData<Entry>),
}

impl<LeafPage, Entry> AnsiBTree<LeafPage, Entry>
where
    LeafPage: AnsiBTreePage<Entry>,
    Entry: BTreeEntry,
{
    pub fn read<R: Read + Seek>(f: &mut R, block: AnsiBlockRef) -> io::Result<Self> {
        f.seek(SeekFrom::Start(u64::from(block.index().index())))?;

        let mut buffer = [0_u8; 512];
        f.read_exact(&mut buffer)?;
        let mut cursor = Cursor::new(buffer);

        cursor.seek(SeekFrom::Start(ANSI_BTREE_ENTRIES_SIZE as u64 + 3))?;
        let level = cursor.read_u8()?;

        cursor.seek(SeekFrom::Start(0))?;
        Ok(if level == 0 {
            AnsiBTree::Leaf(Box::new(LeafPage::read(&mut cursor)?), PhantomData)
        } else {
            AnsiBTree::Intermediate(Box::new(AnsiBTreeEntryPage::read(&mut cursor)?))
        })
    }

    pub fn write<W: Write + Seek>(&self, f: &mut W, block: AnsiBlockRef) -> io::Result<()> {
        f.seek(SeekFrom::Start(u64::from(block.index().index())))?;

        match self {
            AnsiBTree::Intermediate(page) => page.write(f),
            AnsiBTree::Leaf(page, _) => page.write(f),
        }
    }
}

pub type AnsiBlockBTree = AnsiBTree<AnsiBlockBTreePage, AnsiBlockBTreeEntry>;
pub type AnsiNodeBTree = AnsiBTree<AnsiNodeBTreePage, AnsiNodeBTreeEntry>;
