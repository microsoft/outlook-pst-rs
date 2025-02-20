//! [Blocks](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/a9c1981d-d1ea-457c-b39e-dc7fb0eb95d4)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Seek, SeekFrom, Write};

use super::*;
use crate::{
    crc::compute_crc,
    encode::{cyclic, permute},
};

pub const MAX_BLOCK_SIZE: u16 = 8192;

pub const fn block_size(size: u16) -> u16 {
    if size >= MAX_BLOCK_SIZE {
        MAX_BLOCK_SIZE
    } else {
        let size = if size < 64 { 64 } else { size };
        let tail = size % 64;
        if tail == 0 {
            size
        } else {
            size - tail + 64
        }
    }
}

/// [BLOCKTRAILER](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/a14943ef-70c2-403f-898c-5bc3747117e1)
pub trait BlockTrailer: Sized {
    type BlockId: BlockId;
    const SIZE: u16;

    fn new(size: u16, signature: u16, crc: u32, block_id: Self::BlockId) -> NdbResult<Self>;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
    fn size(&self) -> u16;
    fn signature(&self) -> u16;
    fn crc(&self) -> u32;
    fn block_id(&self) -> Self::BlockId;
    fn cyclic_key(&self) -> u32;
}

#[derive(Clone, Copy, Default)]
pub struct UnicodeBlockTrailer {
    size: u16,
    signature: u16,
    crc: u32,
    block_id: UnicodeBlockId,
}

impl BlockTrailer for UnicodeBlockTrailer {
    type BlockId = UnicodeBlockId;
    const SIZE: u16 = 16;

    fn new(size: u16, signature: u16, crc: u32, block_id: UnicodeBlockId) -> NdbResult<Self> {
        if !(1..=(MAX_BLOCK_SIZE - Self::SIZE)).contains(&size) {
            return Err(NdbError::InvalidBlockSize(size));
        }

        Ok(Self {
            size,
            block_id,
            signature,
            crc,
        })
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let size = f.read_u16::<LittleEndian>()?;
        if !(1..=(MAX_BLOCK_SIZE - Self::SIZE)).contains(&size) {
            return Err(NdbError::InvalidBlockSize(size).into());
        }

        let signature = f.read_u16::<LittleEndian>()?;
        let crc = f.read_u32::<LittleEndian>()?;
        let block_id = UnicodeBlockId::read(f)?;

        Ok(Self {
            size,
            signature,
            crc,
            block_id,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.size)?;
        f.write_u16::<LittleEndian>(self.signature)?;
        f.write_u32::<LittleEndian>(self.crc)?;
        self.block_id.write(f)
    }

    fn size(&self) -> u16 {
        self.size
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

    fn cyclic_key(&self) -> u32 {
        u64::from(self.block_id) as u32
    }
}

#[derive(Clone, Copy, Default)]
pub struct AnsiBlockTrailer {
    size: u16,
    signature: u16,
    block_id: AnsiBlockId,
    crc: u32,
}

impl BlockTrailer for AnsiBlockTrailer {
    type BlockId = AnsiBlockId;
    const SIZE: u16 = 12;

    fn new(size: u16, signature: u16, crc: u32, block_id: AnsiBlockId) -> NdbResult<Self> {
        if !(1..=(MAX_BLOCK_SIZE - Self::SIZE)).contains(&size) {
            return Err(NdbError::InvalidBlockSize(size));
        }

        Ok(Self {
            size,
            signature,
            block_id,
            crc,
        })
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let size = f.read_u16::<LittleEndian>()?;
        if !(1..=(MAX_BLOCK_SIZE - Self::SIZE)).contains(&size) {
            return Err(NdbError::InvalidBlockSize(size).into());
        }

        let signature = f.read_u16::<LittleEndian>()?;
        let block_id = AnsiBlockId::read(f)?;
        let crc = f.read_u32::<LittleEndian>()?;

        Ok(Self {
            size,
            signature,
            block_id,
            crc,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.size)?;
        f.write_u16::<LittleEndian>(self.signature)?;
        self.block_id.write(f)?;
        f.write_u32::<LittleEndian>(self.crc)
    }

    fn size(&self) -> u16 {
        self.size
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

    fn cyclic_key(&self) -> u32 {
        u32::from(self.block_id)
    }
}

/// [Data Blocks](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/d0e6fbaf-00e3-4d4d-bea8-8ab3cdb4fde6)
pub trait Block: Sized {
    type Trailer: BlockTrailer;

    fn new(encoding: NdbCryptMethod, data: Vec<u8>, trailer: Self::Trailer) -> NdbResult<Self>;

    fn read<R: Read + Seek>(f: &mut R, size: u16, encoding: NdbCryptMethod) -> io::Result<Self> {
        let mut data = vec![0; size as usize];
        f.read_exact(&mut data)?;

        let offset = i64::from(block_size(size) - size - Self::Trailer::SIZE);
        if offset > 0 {
            f.seek(SeekFrom::Current(offset))?;
        }

        let trailer = Self::Trailer::read(f)?;
        if trailer.size() != size {
            return Err(NdbError::InvalidBlockSize(trailer.size()).into());
        }
        let crc = compute_crc(0, &data);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidBlockCrc(crc).into());
        }

        match encoding {
            NdbCryptMethod::Cyclic => {
                let key = trailer.cyclic_key();
                cyclic::encode_decode_block(&mut data, key);
            }
            NdbCryptMethod::Permute => {
                permute::decode_block(&mut data);
            }
            _ => {}
        }

        Ok(Self::new(encoding, data, trailer)?)
    }

    fn write<W: Write + Seek>(&self, f: &mut W) -> io::Result<()> {
        let mut data = self.data().to_vec();
        let trailer = self.trailer();

        match self.encoding() {
            NdbCryptMethod::Cyclic => {
                let key = trailer.cyclic_key();
                cyclic::encode_decode_block(&mut data, key);
            }
            NdbCryptMethod::Permute => {
                permute::encode_block(&mut data);
            }
            _ => {}
        }

        let crc = compute_crc(0, &data);
        let trailer = Self::Trailer::new(
            data.len() as u16,
            trailer.signature(),
            crc,
            trailer.block_id(),
        )?;

        f.write_all(&data)?;

        let size = data.len() as u16;
        let offset = i64::from(block_size(size) - size - UnicodeBlockTrailer::SIZE);
        if offset > 0 {
            f.seek(SeekFrom::Current(offset))?;
        }

        trailer.write(f)
    }

    fn encoding(&self) -> NdbCryptMethod;
    fn data(&self) -> &[u8];
    fn trailer(&self) -> &Self::Trailer;
}

#[derive(Clone, Default)]
pub struct UnicodeDataBlock {
    encoding: NdbCryptMethod,
    data: Vec<u8>,
    trailer: UnicodeBlockTrailer,
}

impl Block for UnicodeDataBlock {
    type Trailer = UnicodeBlockTrailer;

    fn new(
        encoding: NdbCryptMethod,
        data: Vec<u8>,
        trailer: UnicodeBlockTrailer,
    ) -> NdbResult<Self> {
        let block_id = trailer.block_id();
        if block_id.is_internal() {
            return Err(NdbError::InvalidUnicodeBlockTrailerId(u64::from(block_id)));
        }

        Ok(Self {
            data,
            encoding,
            trailer,
        })
    }

    fn encoding(&self) -> NdbCryptMethod {
        self.encoding
    }

    fn data(&self) -> &[u8] {
        &self.data
    }

    fn trailer(&self) -> &UnicodeBlockTrailer {
        &self.trailer
    }
}

#[derive(Clone, Default)]
pub struct AnsiDataBlock {
    encoding: NdbCryptMethod,
    data: Vec<u8>,
    trailer: AnsiBlockTrailer,
}

impl Block for AnsiDataBlock {
    type Trailer = AnsiBlockTrailer;

    fn new(encoding: NdbCryptMethod, data: Vec<u8>, trailer: AnsiBlockTrailer) -> NdbResult<Self> {
        let block_id = trailer.block_id();
        if block_id.is_internal() {
            return Err(NdbError::InvalidAnsiBlockTrailerId(u32::from(block_id)));
        }

        Ok(Self {
            data,
            encoding,
            trailer,
        })
    }

    fn encoding(&self) -> NdbCryptMethod {
        self.encoding
    }

    fn data(&self) -> &[u8] {
        &self.data
    }

    fn trailer(&self) -> &AnsiBlockTrailer {
        &self.trailer
    }
}

/// [XBLOCK](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/5b7a6935-e83d-4917-9f62-6ce3707f09e0)
/// / [XXBLOCK](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/061b6ac4-d1da-468c-b75d-0303a0a8f468)
pub trait DataTreeBlock: Sized {
    type Trailer: BlockTrailer;
    const ENTRY_SIZE: u16;

    fn level(&self) -> u8;
    fn total_size(&self) -> u32;
    fn entries(&self) -> impl Iterator<Item = <Self::Trailer as BlockTrailer>::BlockId>;
}

trait DataTreeBlockExt: DataTreeBlock {
    fn new(
        data: Vec<u8>,
        level: u8,
        entry_count: u16,
        total_size: u32,
        trailer: Self::Trailer,
    ) -> Self;
    fn verify_internal(
        block_id: <<Self as DataTreeBlock>::Trailer as BlockTrailer>::BlockId,
    ) -> NdbResult<()>;
    fn data(&self) -> &[u8];
    fn trailer(&self) -> &<Self as DataTreeBlock>::Trailer;
}

impl<TreeBlock> Block for TreeBlock
where
    TreeBlock: DataTreeBlockExt,
{
    type Trailer = TreeBlock::Trailer;

    fn new(encoding: NdbCryptMethod, data: Vec<u8>, trailer: Self::Trailer) -> NdbResult<Self> {
        if encoding != NdbCryptMethod::None {
            return Err(NdbError::InvalidInternalBlockEncoding(encoding));
        }

        let block_id = trailer.block_id();
        <Self as DataTreeBlockExt>::verify_internal(block_id)?;

        let mut data = Cursor::new(data);
        let block_type = data.read_u8().map_err(NdbError::InvalidInternalBlockData)?;
        if block_type != 0x01 {
            return Err(NdbError::InvalidInternalBlockType(block_type));
        }

        let level = data.read_u8().map_err(NdbError::InvalidInternalBlockData)?;
        if !(1..=2).contains(&level) {
            return Err(NdbError::InvalidInternalBlockLevel(level));
        }

        let entry_count = data
            .read_u16::<LittleEndian>()
            .map_err(NdbError::InvalidInternalBlockData)?;
        if entry_count
            > (trailer.size() - Self::Trailer::SIZE - DATA_TREE_BLOCK_HEADER_SIZE)
                / <Self as DataTreeBlock>::ENTRY_SIZE
        {
            return Err(NdbError::InvalidInternalBlockEntryCount(entry_count));
        }
        let total_size = data
            .read_u32::<LittleEndian>()
            .map_err(NdbError::InvalidInternalBlockData)?;

        let data = data.into_inner();

        Ok(<Self as DataTreeBlockExt>::new(
            data,
            level,
            entry_count,
            total_size,
            trailer,
        ))
    }

    fn encoding(&self) -> NdbCryptMethod {
        NdbCryptMethod::None
    }

    fn data(&self) -> &[u8] {
        <Self as DataTreeBlockExt>::data(self)
    }

    fn trailer(&self) -> &Self::Trailer {
        <Self as DataTreeBlockExt>::trailer(self)
    }
}

const DATA_TREE_BLOCK_HEADER_SIZE: u16 = 8;

#[derive(Clone, Default)]
pub struct UnicodeDataTreeBlock {
    data: Vec<u8>,
    level: u8,
    entry_count: u16,
    total_size: u32,
    trailer: UnicodeBlockTrailer,
}

impl UnicodeDataTreeBlock {
    pub fn new(data: Vec<u8>, trailer: <Self as DataTreeBlock>::Trailer) -> NdbResult<Self> {
        <Self as Block>::new(NdbCryptMethod::None, data, trailer)
    }

    pub fn read<R: Read + Seek>(f: &mut R, size: u16) -> io::Result<Self> {
        <Self as Block>::read(f, size, NdbCryptMethod::None)
    }

    pub fn write<W: Write + Seek>(&self, f: &mut W) -> io::Result<()> {
        <Self as Block>::write(self, f)
    }
}

impl DataTreeBlock for UnicodeDataTreeBlock {
    type Trailer = UnicodeBlockTrailer;
    const ENTRY_SIZE: u16 = 8;

    fn level(&self) -> u8 {
        self.level
    }

    fn total_size(&self) -> u32 {
        self.total_size
    }

    fn entries(&self) -> impl Iterator<Item = <Self::Trailer as BlockTrailer>::BlockId> {
        let data = self.data.as_slice()[(DATA_TREE_BLOCK_HEADER_SIZE as usize)
            ..((self.entry_count * Self::ENTRY_SIZE) as usize)]
            .to_vec();
        let mut data = Cursor::new(data);

        (0..self.entry_count)
            .filter_map(move |_| <Self::Trailer as BlockTrailer>::BlockId::read(&mut data).ok())
            .fuse()
    }
}

impl DataTreeBlockExt for UnicodeDataTreeBlock {
    fn new(
        data: Vec<u8>,
        level: u8,
        entry_count: u16,
        total_size: u32,
        trailer: Self::Trailer,
    ) -> Self {
        Self {
            data,
            level,
            entry_count,
            total_size,
            trailer,
        }
    }

    fn data(&self) -> &[u8] {
        &self.data
    }

    fn verify_internal(block_id: <Self::Trailer as BlockTrailer>::BlockId) -> NdbResult<()> {
        if block_id.is_internal() {
            Ok(())
        } else {
            Err(NdbError::InvalidUnicodeBlockTrailerId(u64::from(block_id)))
        }
    }

    fn trailer(&self) -> &Self::Trailer {
        &self.trailer
    }
}

#[derive(Clone, Default)]
pub struct AnsiDataTreeBlock {
    data: Vec<u8>,
    level: u8,
    entry_count: u16,
    total_size: u32,
    trailer: AnsiBlockTrailer,
}

impl AnsiDataTreeBlock {
    pub fn new(data: Vec<u8>, trailer: <Self as DataTreeBlock>::Trailer) -> NdbResult<Self> {
        <Self as Block>::new(NdbCryptMethod::None, data, trailer)
    }

    pub fn read<R: Read + Seek>(f: &mut R, size: u16) -> io::Result<Self> {
        <Self as Block>::read(f, size, NdbCryptMethod::None)
    }

    pub fn write<W: Write + Seek>(&self, f: &mut W) -> io::Result<()> {
        <Self as Block>::write(self, f)
    }
}

impl DataTreeBlock for AnsiDataTreeBlock {
    type Trailer = AnsiBlockTrailer;
    const ENTRY_SIZE: u16 = 4;

    fn level(&self) -> u8 {
        self.level
    }

    fn total_size(&self) -> u32 {
        self.total_size
    }

    fn entries(&self) -> impl Iterator<Item = <Self::Trailer as BlockTrailer>::BlockId> {
        let data = self.data.as_slice()[(DATA_TREE_BLOCK_HEADER_SIZE as usize)
            ..((self.entry_count * Self::ENTRY_SIZE) as usize)]
            .to_vec();
        let mut data = Cursor::new(data);

        (0..self.entry_count)
            .filter_map(move |_| <Self::Trailer as BlockTrailer>::BlockId::read(&mut data).ok())
            .fuse()
    }
}

impl DataTreeBlockExt for AnsiDataTreeBlock {
    fn new(
        data: Vec<u8>,
        level: u8,
        entry_count: u16,
        total_size: u32,
        trailer: Self::Trailer,
    ) -> Self {
        Self {
            data,
            level,
            entry_count,
            total_size,
            trailer,
        }
    }

    fn data(&self) -> &[u8] {
        &self.data
    }

    fn verify_internal(block_id: <Self::Trailer as BlockTrailer>::BlockId) -> NdbResult<()> {
        if block_id.is_internal() {
            Ok(())
        } else {
            Err(NdbError::InvalidAnsiBlockTrailerId(u32::from(block_id)))
        }
    }

    fn trailer(&self) -> &Self::Trailer {
        &self.trailer
    }
}
