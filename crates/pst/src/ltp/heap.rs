//! ## [HN (Heap-on-Node)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/77ce49a3-3772-4d8d-bb2c-2f7520a238a6)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

use super::{read_write::*, *};
use crate::ndb::{
    block::*,
    header::NdbCryptMethod,
    node_id::*,
    page::{AnsiBlockBTree, UnicodeBlockBTree},
    read_write::*,
};

pub const HEAP_INDEX_MASK: u32 = (1_u16.rotate_right(5) - 1) as u32;

/// [HID](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/85b9e985-ea53-447f-b70c-eb82bfbdcbc9)
#[derive(Clone, Copy, Default, Debug)]
pub struct HeapId(NodeId);

impl HeapId {
    pub fn new(index: u16, block_index: u16) -> LtpResult<Self> {
        let shifted_index = index.rotate_left(11);
        if shifted_index & 0x1F != 0 {
            return Err(LtpError::InvalidHeapIndex(index));
        };

        let node_index = ((block_index as u32) << 11) | index as u32;

        Ok(Self(NodeId::new(NodeIdType::HeapNode, node_index)?))
    }

    pub fn index(&self) -> LtpResult<u16> {
        let index = (self.0.index() & HEAP_INDEX_MASK) as u16;
        if index < 1 {
            return Err(LtpError::InvalidHeapIndex(index));
        }
        Ok(index - 1)
    }

    pub fn block_index(&self) -> u16 {
        (self.0.index() >> 11) as u16
    }
}

impl HeapIdReadWrite for HeapId {
    fn new(index: u16, block_index: u16) -> LtpResult<Self> {
        Self::new(index, block_index)
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = NodeId::read(f)?;

        let id_type = value.id_type()?;
        if id_type != NodeIdType::HeapNode {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                LtpError::InvalidNodeType(id_type),
            ));
        }

        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.0.write(f)
    }
}

impl From<u32> for HeapId {
    fn from(value: u32) -> Self {
        Self(NodeId::from(value))
    }
}

impl From<HeapId> for u32 {
    fn from(value: HeapId) -> Self {
        u32::from(value.0)
    }
}

/// `bClientSig`
///
/// ### See also
/// [HeapNodeHeader]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HeapNodeType {
    /// `bTypeReserved1`: Reserved
    Reserved1 = 0x6C,
    /// `bTypeTC`: Table Context (TC/HN)
    Table = 0x7C,
    /// `bTypeReserved2`: Reserved
    Reserved2 = 0x8C,
    /// `bTypeReserved3`: Reserved
    Reserved3 = 0x9C,
    /// `bTypeReserved4`: Reserved
    Reserved4 = 0xA5,
    /// `bTypeReserved5`: Reserved
    Reserved5 = 0xAC,
    /// `bTypeBTH`: BTree-on-Heap (BTH)
    Tree = 0xB5,
    /// `bTypePC`: Property Context (PC/BTH)
    Properties = 0xBC,
    /// `bTypeReserved6`: Reserved
    Reserved6 = 0xCC,
}

impl TryFrom<u8> for HeapNodeType {
    type Error = LtpError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x6C => Ok(Self::Reserved1),
            0x7C => Ok(Self::Table),
            0x8C => Ok(Self::Reserved2),
            0x9C => Ok(Self::Reserved3),
            0xA5 => Ok(Self::Reserved4),
            0xAC => Ok(Self::Reserved5),
            0xB5 => Ok(Self::Tree),
            0xBC => Ok(Self::Properties),
            0xCC => Ok(Self::Reserved6),
            _ => Err(LtpError::InvalidHeapNodeTypeSignature(value)),
        }
    }
}

/// `rgbFillLevel`
///
/// ### See also
/// [HeapNodeHeader]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HeapFillLevel {
    /// `FILL_LEVEL_EMPTY`: At least 3584 bytes free / data block does not exist
    Empty = 0x00,
    /// `FILL_LEVEL_1`: 2560-3584 bytes free
    Level1 = 0x01,
    /// `FILL_LEVEL_2`: 2048-2560 bytes free
    Level2 = 0x02,
    /// `FILL_LEVEL_3`: 1792-2048 bytes free
    Level3 = 0x03,
    /// `FILL_LEVEL_4`: 1536-1792 bytes free
    Level4 = 0x04,
    /// `FILL_LEVEL_5`: 1280-1536 bytes free
    Level5 = 0x05,
    /// `FILL_LEVEL_6`: 1024-1280 bytes free
    Level6 = 0x06,
    /// `FILL_LEVEL_7`: 768-1024 bytes free
    Level7 = 0x07,
    /// `FILL_LEVEL_8`: 512-768 bytes free
    Level8 = 0x08,
    /// `FILL_LEVEL_9`: 256-512 bytes free
    Level9 = 0x09,
    /// `FILL_LEVEL_10`: 128-256 bytes free
    Level10 = 0x0A,
    /// `FILL_LEVEL_11`: 64-128 bytes free
    Level11 = 0x0B,
    /// `FILL_LEVEL_12`: 32-64 bytes free
    Level12 = 0x0C,
    /// `FILL_LEVEL_13`: 16-32 bytes free
    Level13 = 0x0D,
    /// `FILL_LEVEL_14`: 8-16 bytes free
    Level14 = 0x0E,
    /// `FILL_LEVEL_15`: Data block has less than 8 bytes free
    Level15 = 0x0F,
}

impl TryFrom<u8> for HeapFillLevel {
    type Error = LtpError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Empty),
            0x01 => Ok(Self::Level1),
            0x02 => Ok(Self::Level2),
            0x03 => Ok(Self::Level3),
            0x04 => Ok(Self::Level4),
            0x05 => Ok(Self::Level5),
            0x06 => Ok(Self::Level6),
            0x07 => Ok(Self::Level7),
            0x08 => Ok(Self::Level8),
            0x09 => Ok(Self::Level9),
            0x0A => Ok(Self::Level10),
            0x0B => Ok(Self::Level11),
            0x0C => Ok(Self::Level12),
            0x0D => Ok(Self::Level13),
            0x0E => Ok(Self::Level14),
            0x0F => Ok(Self::Level15),
            _ => Err(LtpError::InvalidHeapFillLevel(value)),
        }
    }
}

impl HeapFillLevel {
    fn unpack_fill_levels(value: u32) -> [HeapFillLevel; 8] {
        [
            HeapFillLevel::try_from((value & 0x0F) as u8).unwrap(),
            HeapFillLevel::try_from(((value >> 4) & 0x0F) as u8).unwrap(),
            HeapFillLevel::try_from(((value >> 8) & 0x0F) as u8).unwrap(),
            HeapFillLevel::try_from(((value >> 12) & 0x0F) as u8).unwrap(),
            HeapFillLevel::try_from(((value >> 16) & 0x0F) as u8).unwrap(),
            HeapFillLevel::try_from(((value >> 20) & 0x0F) as u8).unwrap(),
            HeapFillLevel::try_from(((value >> 24) & 0x0F) as u8).unwrap(),
            HeapFillLevel::try_from(((value >> 28) & 0x0F) as u8).unwrap(),
        ]
    }

    fn pack_fill_levels(fill_levels: &[HeapFillLevel; 8]) -> u32 {
        fill_levels
            .iter()
            .fold(0, |acc, &x| (acc << 4) | (x as u32))
    }
}

/// [HNHDR](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/8e4ae05c-3c24-4103-b7e5-ffef6f244834)
#[derive(Clone, Copy, Debug)]
pub struct HeapNodeHeader {
    page_map_offset: u16,
    client_signature: HeapNodeType,
    user_root: HeapId,
    fill_levels: [HeapFillLevel; 8],
}

impl HeapNodeHeader {
    pub fn new(
        page_map_offset: u16,
        client_signature: HeapNodeType,
        user_root: HeapId,
        fill_levels: [HeapFillLevel; 8],
    ) -> Self {
        Self {
            page_map_offset,
            client_signature,
            user_root,
            fill_levels,
        }
    }

    pub fn page_map_offset(&self) -> u16 {
        self.page_map_offset
    }

    pub fn client_signature(&self) -> HeapNodeType {
        self.client_signature
    }

    pub fn user_root(&self) -> HeapId {
        self.user_root
    }

    pub fn fill_levels(&self) -> &[HeapFillLevel; 8] {
        &self.fill_levels
    }
}

impl HeapNodeReadWrite for HeapNodeHeader {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let page_map_offset = f.read_u16::<LittleEndian>()?;
        let heap_signature = f.read_u8()?;
        if heap_signature != 0xEC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                LtpError::InvalidHeapNodeSignature(heap_signature),
            ));
        }
        let client_signature = HeapNodeType::try_from(f.read_u8()?)?;
        let user_root = HeapId::read(f)?;
        let fill_levels = HeapFillLevel::unpack_fill_levels(f.read_u32::<LittleEndian>()?);

        Ok(Self::new(
            page_map_offset,
            client_signature,
            user_root,
            fill_levels,
        ))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.page_map_offset)?;
        f.write_u8(0xEC)?;
        f.write_u8(self.client_signature as u8)?;
        self.user_root.write(f)?;
        let fill_levels = HeapFillLevel::pack_fill_levels(&self.fill_levels);
        f.write_u32::<LittleEndian>(fill_levels)
    }
}

/// [HNPAGEHDR](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/9c34ecf8-36bc-45a1-a2df-ee35c6dc840a)
#[derive(Clone, Copy, Debug)]
pub struct HeapNodePageHeader(u16);

impl HeapNodePageHeader {
    pub fn new(page_index: u16) -> Self {
        Self(page_index)
    }

    pub fn page_map_offset(&self) -> u16 {
        self.0
    }
}

impl HeapNodeReadWrite for HeapNodePageHeader {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let page_index = f.read_u16::<LittleEndian>()?;
        Ok(Self::new(page_index))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.0)
    }
}

/// [HNBITMAPHDR](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/822e2327-b29d-4ec4-91be-45637a438d40)
#[derive(Clone, Copy, Debug)]
pub struct HeapNodeBitmapHeader {
    page_map_offset: u16,
    fill_levels: [HeapFillLevel; 128],
}

impl HeapNodeBitmapHeader {
    pub fn new(page_map_offset: u16, fill_levels: [HeapFillLevel; 128]) -> Self {
        Self {
            page_map_offset,
            fill_levels,
        }
    }

    pub fn page_map_offset(&self) -> u16 {
        self.page_map_offset
    }

    pub fn fill_levels(&self) -> &[HeapFillLevel; 128] {
        &self.fill_levels
    }
}

impl HeapNodeReadWrite for HeapNodeBitmapHeader {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let page_map_offset = f.read_u16::<LittleEndian>()?;

        let mut fill_levels = [HeapFillLevel::Empty; 128];
        for i in 0..16 {
            let unpacked = HeapFillLevel::unpack_fill_levels(f.read_u32::<LittleEndian>()?);
            fill_levels[i * 8..(i + 1) * 8].copy_from_slice(&unpacked);
        }

        Ok(Self::new(page_map_offset, fill_levels))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.page_map_offset)?;
        for i in 0..16 {
            let mut packed = [HeapFillLevel::Empty; 8];
            packed.copy_from_slice(&self.fill_levels[i * 8..(i + 1) * 8]);
            f.write_u32::<LittleEndian>(HeapFillLevel::pack_fill_levels(&packed))?;
        }
        Ok(())
    }
}

pub struct HeapNodePageAllocOffsets(Vec<u16>);

impl HeapNodePageAllocOffsets {
    pub fn new(offsets: Vec<u16>) -> Self {
        Self(offsets)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct HeapNodePageAlloc {
    offset: u16,
    size: u16,
}

impl HeapNodePageAlloc {
    pub fn offset(&self) -> u16 {
        self.offset
    }

    pub fn size(&self) -> u16 {
        self.size
    }
}

/// [HNPAGEMAP](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/291653c0-b347-4c5b-ba41-85ad780b4ba4)
#[derive(Clone, Default, Debug)]
pub struct HeapNodePageMap {
    allocations: Vec<HeapNodePageAlloc>,
    next_offset: u16,
    free_count: u16,
}

impl HeapNodePageMap {
    pub fn new(
        alloc_count: u16,
        free_count: u16,
        offsets: HeapNodePageAllocOffsets,
    ) -> LtpResult<Self> {
        let page_map = Self::try_from(offsets)?;

        if alloc_count as usize != page_map.allocations.len() {
            return Err(LtpError::InvalidHeapPageAllocCount(alloc_count));
        }

        if free_count != page_map.free_count {
            return Err(LtpError::InvalidHeapPageFreeCount(free_count));
        }

        Ok(page_map)
    }

    pub fn allocations(&self) -> &[HeapNodePageAlloc] {
        &self.allocations
    }

    pub fn next_offset(&self) -> u16 {
        self.next_offset
    }
}

impl TryFrom<HeapNodePageAllocOffsets> for HeapNodePageMap {
    type Error = LtpError;

    fn try_from(offsets: HeapNodePageAllocOffsets) -> Result<Self, Self::Error> {
        let mut offset = None;
        let mut free_count = 0;
        let allocations = offsets
            .0
            .into_iter()
            .filter_map(|next_offset| {
                let Some(last_offset) = offset else {
                    offset = Some(next_offset);
                    return None;
                };

                if next_offset < last_offset {
                    return Some(Err(LtpError::InvalidHeapPageAllocOffset(next_offset)));
                }

                let size = next_offset - last_offset;
                let alloc = HeapNodePageAlloc {
                    offset: last_offset,
                    size,
                };

                if size == 0 {
                    let Ok(value) = u16::try_from(1_u32 + u32::from(free_count)) else {
                        return Some(Err(LtpError::HeapPageOutOfSpace));
                    };

                    free_count = value;
                } else {
                    offset = Some(next_offset);
                }

                Some(Ok(alloc))
            })
            .collect::<LtpResult<_>>()?;

        let next_offset = offset.ok_or(LtpError::EmptyHeapPageAlloc)?;

        Ok(Self {
            allocations,
            next_offset,
            free_count,
        })
    }
}

impl HeapNodeReadWrite for HeapNodePageMap {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let alloc_count = f.read_u16::<LittleEndian>()?;
        let free_count = f.read_u16::<LittleEndian>()?;
        let mut allocations = Vec::with_capacity(alloc_count as usize + 1);
        for _ in 0..=alloc_count {
            let offset = f.read_u16::<LittleEndian>()?;
            allocations.push(offset);
        }

        Ok(Self::new(
            alloc_count,
            free_count,
            HeapNodePageAllocOffsets::new(allocations),
        )?)
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        let alloc_count =
            u16::try_from(self.allocations.len()).map_err(|_| LtpError::HeapPageOutOfSpace)?;
        f.write_u16::<LittleEndian>(alloc_count)?;
        f.write_u16::<LittleEndian>(self.free_count)?;
        let mut last_offset = 0;
        for HeapNodePageAlloc { offset, size } in &self.allocations {
            f.write_u16::<LittleEndian>(*offset)?;
            last_offset = *offset + *size;
        }
        f.write_u16::<LittleEndian>(last_offset)
    }
}

pub struct UnicodeHeapNode {
    data: UnicodeDataTree,
}

impl UnicodeHeapNode {
    pub fn new(data: UnicodeDataTree) -> Self {
        Self { data }
    }

    pub fn data(&self) -> &UnicodeDataTree {
        &self.data
    }

    pub fn header<R: Read + Seek>(
        &self,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_tree: &UnicodeBlockBTree,
    ) -> io::Result<HeapNodeHeader> {
        let block = self
            .data
            .blocks(f, encoding, block_tree)?
            .next()
            .ok_or(LtpError::HeapBlockIndexNotFound(0))?;

        let mut cursor = block.data();
        let header = HeapNodeHeader::read(&mut cursor)?;
        Ok(header)
    }

    pub fn find_entry<R: Read + Seek>(
        &self,
        heap_id: HeapId,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_tree: &UnicodeBlockBTree,
    ) -> io::Result<Vec<u8>> {
        let block_index = heap_id.block_index();
        let block = self
            .data
            .blocks(f, encoding, block_tree)?
            .nth(block_index as usize)
            .ok_or(LtpError::HeapBlockIndexNotFound(block_index))?;

        let mut cursor = Cursor::new(block.data());

        let page_map_offset = match block_index {
            0 => {
                let header = HeapNodeHeader::read(&mut cursor)?;
                header.page_map_offset()
            }
            bitmap if bitmap >= 8 && (bitmap - 8) % 128 == 0 => {
                let header = HeapNodeBitmapHeader::read(&mut cursor)?;
                header.page_map_offset()
            }
            _ => {
                let header = HeapNodePageHeader::read(&mut cursor)?;
                header.page_map_offset()
            }
        };

        cursor.seek(SeekFrom::Start(u64::from(page_map_offset)))?;
        let page_map = HeapNodePageMap::read(&mut cursor)?;
        let allocations = page_map.allocations();

        let index = heap_id.index()?;
        if index as usize >= allocations.len() {
            return Err(LtpError::HeapAllocIndexNotFound(index).into());
        }

        let alloc = &allocations[index as usize];
        let start = alloc.offset() as usize;
        let end = start + alloc.size() as usize;
        Ok(block.data()[start..end].to_vec())
    }
}

pub struct AnsiHeapNode {
    data: AnsiDataTree,
}

impl AnsiHeapNode {
    pub fn new(data: AnsiDataTree) -> Self {
        Self { data }
    }

    pub fn data(&self) -> &AnsiDataTree {
        &self.data
    }

    pub fn header<R: Read + Seek>(
        &self,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_tree: &AnsiBlockBTree,
    ) -> io::Result<HeapNodeHeader> {
        let block = self
            .data
            .blocks(f, encoding, block_tree)?
            .next()
            .ok_or(LtpError::HeapBlockIndexNotFound(0))?;

        let mut cursor = block.data();
        let header = HeapNodeHeader::read(&mut cursor)?;
        Ok(header)
    }

    pub fn find_entry<R: Read + Seek>(
        &self,
        heap_id: HeapId,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_tree: &AnsiBlockBTree,
    ) -> io::Result<Vec<u8>> {
        let block_index = heap_id.block_index();
        let block = self
            .data
            .blocks(f, encoding, block_tree)?
            .nth(block_index as usize)
            .ok_or(LtpError::HeapBlockIndexNotFound(block_index))?;

        let mut cursor = Cursor::new(block.data());

        let page_map_offset = match block_index {
            0 => {
                let header = HeapNodeHeader::read(&mut cursor)?;
                header.page_map_offset()
            }
            bitmap if (bitmap - 8) % 128 == 0 => {
                let header = HeapNodeBitmapHeader::read(&mut cursor)?;
                header.page_map_offset()
            }
            _ => {
                let header = HeapNodePageHeader::read(&mut cursor)?;
                header.page_map_offset()
            }
        };

        cursor.seek(SeekFrom::Start(u64::from(page_map_offset)))?;
        let page_map = HeapNodePageMap::read(&mut cursor)?;
        let allocations = page_map.allocations();

        let index = heap_id.index()?;
        if index as usize >= allocations.len() {
            return Err(LtpError::HeapAllocIndexNotFound(index).into());
        }

        let alloc = &allocations[index as usize];
        let start = alloc.offset() as usize;
        let end = start + alloc.size() as usize;
        Ok(block.data()[start..end].to_vec())
    }
}
