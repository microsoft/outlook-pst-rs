#![allow(dead_code)]

use std::{
    cmp::Ordering,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
};

use super::{
    block::*, block_id::*, block_ref::*, byte_index::*, header::*, node_id::*, page::*, root::*, *,
};
use crate::{
    crc::compute_crc,
    encode::{cyclic, permute},
};

pub trait NodeIdReadWrite: Copy + Sized {
    fn new(id_type: NodeIdType, index: u32) -> NdbResult<Self>;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait BlockIdReadWrite: BlockId + Copy + Sized {
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait ByteIndexReadWrite: ByteIndex + Copy + Sized {
    fn new(index: Self::Index) -> Self;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait BlockRefReadWrite: BlockRef + Copy + Sized
where
    <Self as BlockRef>::Block: BlockIdReadWrite,
    <Self as BlockRef>::Index: ByteIndexReadWrite,
{
    fn new(block: Self::Block, index: Self::Index) -> Self;

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

pub trait RootReadWrite: Root + Sized
where
    <Self as Root>::Index: ByteIndexReadWrite,
    <Self as Root>::BTreeRef:
        BlockRef<Block: BlockIdReadWrite, Index: ByteIndexReadWrite> + BlockRefReadWrite,
{
    fn new(
        file_eof_index: Self::Index,
        amap_last_index: Self::Index,
        amap_free_size: Self::Index,
        pmap_free_size: Self::Index,
        node_btree: Self::BTreeRef,
        block_btree: Self::BTreeRef,
        amap_is_valid: AmapStatus,
    ) -> Self;

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

pub trait HeaderReadWrite: Header + Sized
where
    <Self as Header>::Root: Root<
            Index: ByteIndexReadWrite,
            BTreeRef: BlockRef<Block: BlockIdReadWrite, Index: ByteIndexReadWrite>
                          + BlockRefReadWrite,
        > + RootReadWrite,
{
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait PageTrailerReadWrite: PageTrailer + Copy + Sized {
    fn new(page_type: PageType, signature: u16, block_id: Self::BlockId, crc: u32) -> Self;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait MapPageReadWrite: MapPage + Sized {
    const PAGE_TYPE: u8;

    fn new(amap_bits: MapBits, trailer: Self::Trailer) -> NdbResult<Self>;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait DensityListPageReadWrite: DensityListPage + Sized {
    fn new(
        backfill_complete: bool,
        current_page: u32,
        entries: &[DensityListPageEntry],
        trailer: Self::Trailer,
    ) -> NdbResult<Self>;
    fn read<R: Read + Seek>(f: &mut R) -> io::Result<Self>;
    fn write<W: Write + Seek>(&self, f: &mut W) -> io::Result<()>;
}

pub trait BTreePageKeyReadWrite: Copy + Sized {
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait BTreeEntryReadWrite: BTreeEntry + Copy + Sized + Default {
    const ENTRY_SIZE: usize;

    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait BlockBTreeEntryReadWrite: BlockBTreeEntry + Copy + Sized {
    fn new(block: Self::Block, size: u16) -> Self;
}

pub trait BTreePageEntryReadWrite: BTreePageEntry
where
    Self: BTreeEntryReadWrite,
    <Self as BTreeEntry>::Key: BTreePageKeyReadWrite,
    <Self as BTreePageEntry>::Block:
        BlockRef<Block: BlockIdReadWrite, Index: ByteIndexReadWrite> + BlockRefReadWrite,
{
    const ENTRY_SIZE: usize;

    fn new(key: Self::Key, block: Self::Block) -> Self;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        Ok(Self::new(Self::Key::read(f)?, Self::Block::read(f)?))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.key().write(f)?;
        self.block().write(f)
    }
}

pub trait BTreePageReadWrite: BTreePage + Sized {
    fn new(
        level: u8,
        max_entries: u8,
        entry_size: u8,
        entries: &[Self::Entry],
        trailer: Self::Trailer,
    ) -> NdbResult<Self>;

    fn max_entries(&self) -> u8;
    fn entry_size(&self) -> u8;
}

pub const UNICODE_BTREE_ENTRIES_SIZE: usize = 488;

pub trait UnicodeBTreePageReadWrite<Entry>:
    BTreePageReadWrite<Entry = Entry, Trailer = UnicodePageTrailer> + Sized
where
    Entry: BTreeEntryReadWrite,
{
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let mut buffer = [0_u8; 496];
        f.read_exact(&mut buffer)?;
        let buffer = buffer.as_slice();

        let mut cursor = Cursor::new(&buffer[UNICODE_BTREE_ENTRIES_SIZE..]);

        // cEnt
        let entry_count = usize::from(cursor.read_u8()?);

        // cEntMax
        let max_entries = cursor.read_u8()?;

        // cbEnt
        let entry_size = cursor.read_u8()?;

        if entry_count > usize::from(max_entries) {
            return Err(NdbError::InvalidBTreeEntryCount(entry_count).into());
        }
        if usize::from(entry_size) < Entry::ENTRY_SIZE {
            return Err(NdbError::InvalidBTreeEntrySize(entry_size).into());
        }
        if usize::from(max_entries) > UNICODE_BTREE_ENTRIES_SIZE / usize::from(entry_size) {
            return Err(NdbError::InvalidBTreeEntryMaxCount(max_entries).into());
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

        let crc = compute_crc(0, buffer);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidPageCrc(crc).into());
        }

        // rgentries
        let mut entries = Vec::with_capacity(entry_count);
        for i in 0..entry_count {
            let offset = i * usize::from(entry_size);
            let end = offset + usize::from(entry_size);
            let mut cursor = &buffer[offset..end];
            entries.push(<Self::Entry as BTreeEntryReadWrite>::read(&mut cursor)?);
        }

        Ok(<Self as BTreePageReadWrite>::new(
            level,
            max_entries,
            entry_size,
            &entries,
            trailer,
        )?)
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        let mut buffer = [0_u8; 496];

        // rgentries
        let entries = self.entries();
        for (i, entry) in entries.iter().enumerate() {
            let offset = i * usize::from(self.entry_size());
            let end = offset + usize::from(self.entry_size());
            let mut cursor = &mut buffer[offset..end];
            <Self::Entry as BTreeEntryReadWrite>::write(entry, &mut cursor)?;
        }

        let mut cursor = Cursor::new(&mut buffer[UNICODE_BTREE_ENTRIES_SIZE..]);

        // cEnt
        cursor.write_u8(entries.len() as u8)?;

        // cEntMax
        cursor.write_u8(self.max_entries())?;

        // cbEnt
        cursor.write_u8(self.entry_size())?;

        // cLevel
        cursor.write_u8(self.level())?;

        // dwPadding
        cursor.write_u32::<LittleEndian>(0)?;

        cursor.flush()?;
        let crc = compute_crc(0, &buffer);

        f.write_all(&buffer)?;

        // pageTrailer
        let trailer = self.trailer();
        let trailer = UnicodePageTrailer::new(
            trailer.page_type(),
            trailer.signature(),
            trailer.block_id(),
            crc,
        );

        trailer.write(f)
    }
}

pub const ANSI_BTREE_ENTRIES_SIZE: usize = 496;

pub trait AnsiBTreePageReadWrite<Entry>:
    BTreePageReadWrite<Entry = Entry, Trailer = AnsiPageTrailer> + Sized
where
    Entry: BTreeEntryReadWrite,
{
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let mut buffer = [0_u8; 500];
        f.read_exact(&mut buffer)?;
        let buffer = buffer.as_slice();

        let mut cursor = Cursor::new(&buffer[ANSI_BTREE_ENTRIES_SIZE..]);

        // cEnt
        let entry_count = usize::from(cursor.read_u8()?);

        // cEntMax
        let max_entries = cursor.read_u8()?;

        // cbEnt
        let entry_size = cursor.read_u8()?;

        if entry_count > usize::from(max_entries) {
            return Err(NdbError::InvalidBTreeEntryCount(entry_count).into());
        }
        if usize::from(entry_size) < Entry::ENTRY_SIZE {
            return Err(NdbError::InvalidBTreeEntrySize(entry_size).into());
        }
        if usize::from(max_entries) > ANSI_BTREE_ENTRIES_SIZE / usize::from(entry_size) {
            return Err(NdbError::InvalidBTreeEntryMaxCount(max_entries).into());
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

        let crc = compute_crc(0, buffer);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidPageCrc(crc).into());
        }

        // rgentries
        let mut entries = Vec::with_capacity(entry_count);
        for i in 0..entry_count {
            let offset = i * usize::from(entry_size);
            let end = offset + usize::from(entry_size);
            let mut cursor = &buffer[offset..end];
            entries.push(<Self::Entry as BTreeEntryReadWrite>::read(&mut cursor)?);
        }

        Ok(<Self as BTreePageReadWrite>::new(
            level,
            max_entries,
            entry_size,
            &entries,
            trailer,
        )?)
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        let mut buffer = [0_u8; 500];

        // rgentries
        let entries = self.entries();
        for (i, entry) in entries.iter().enumerate() {
            let offset = i * usize::from(self.entry_size());
            let end = offset + usize::from(self.entry_size());
            let mut cursor = &mut buffer[offset..end];
            <Self::Entry as BTreeEntryReadWrite>::write(entry, &mut cursor)?;
        }

        let mut cursor = Cursor::new(&mut buffer[ANSI_BTREE_ENTRIES_SIZE..]);

        // cEnt
        cursor.write_u8(entries.len() as u8)?;

        // cEntMax
        cursor.write_u8(self.max_entries())?;

        // cbEnt
        cursor.write_u8(self.entry_size())?;

        // cLevel
        cursor.write_u8(self.level())?;

        cursor.flush()?;
        let crc = compute_crc(0, &buffer);

        f.write_all(&buffer)?;

        // pageTrailer
        let trailer = self.trailer();
        let trailer = AnsiPageTrailer::new(
            trailer.page_type(),
            trailer.signature(),
            trailer.block_id(),
            crc,
        );
        trailer.write(f)
    }
}

pub trait NodeBTreeEntryReadWrite: NodeBTreeEntry + BTreeEntryReadWrite {
    fn new(
        node: NodeId,
        data: <Self as NodeBTreeEntry>::Block,
        sub_node: Option<<Self as NodeBTreeEntry>::Block>,
        parent: Option<NodeId>,
    ) -> Self;
}

pub trait BlockTrailerReadWrite: BlockTrailer + Copy + Sized {
    const SIZE: u16;

    fn new(size: u16, signature: u16, crc: u32, block_id: Self::BlockId) -> NdbResult<Self>;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait BlockReadWrite: Block + Sized
where
    <Self as Block>::Trailer: BlockTrailerReadWrite,
{
    fn new(encoding: NdbCryptMethod, data: Vec<u8>, trailer: Self::Trailer) -> NdbResult<Self>;

    fn read<R: Read + Seek>(f: &mut R, size: u16, encoding: NdbCryptMethod) -> io::Result<Self> {
        let mut data = vec![0; size as usize];
        f.read_exact(&mut data)?;

        let offset = size + Self::Trailer::SIZE;
        let offset = i64::from(block_size(offset) - offset);
        if offset > 0 {
            f.seek(SeekFrom::Current(offset))?;
        }

        let trailer = Self::Trailer::read(f)?;
        if trailer.size() != size {
            return Err(NdbError::InvalidBlockSize(trailer.size()).into());
        }
        trailer.verify_block_id(false)?;
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
        let offset = size + Self::Trailer::SIZE;
        let offset = i64::from(block_size(offset) - offset);
        if offset > 0 {
            f.seek(SeekFrom::Current(offset))?;
        }

        trailer.write(f)
    }
}

pub trait IntermediateTreeHeaderReadWrite: IntermediateTreeHeader + Copy + Sized {
    const HEADER_SIZE: u16;

    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait IntermediateTreeEntryReadWrite: IntermediateTreeEntry + Copy + Sized {
    const ENTRY_SIZE: u16;

    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait IntermediateTreeBlockReadWrite: IntermediateTreeBlock + Sized
where
    <Self as IntermediateTreeBlock>::Header: IntermediateTreeHeaderReadWrite,
    <Self as IntermediateTreeBlock>::Entry: IntermediateTreeEntryReadWrite,
    <Self as IntermediateTreeBlock>::Trailer: BlockTrailerReadWrite,
{
    fn new(
        header: Self::Header,
        entries: Vec<Self::Entry>,
        trailer: Self::Trailer,
    ) -> NdbResult<Self>;

    fn read<R: Read + Seek>(f: &mut R, header: Self::Header, size: u16) -> io::Result<Self> {
        let mut data = vec![0; size as usize];
        f.read_exact(&mut data)?;
        let mut cursor = Cursor::new(&data[Self::Header::HEADER_SIZE as usize..]);

        let entry_count = header.entry_count();

        if entry_count * Self::Entry::ENTRY_SIZE > size - Self::Header::HEADER_SIZE {
            return Err(NdbError::InvalidInternalBlockEntryCount(entry_count).into());
        }

        let entries = (0..entry_count)
            .map(move |_| <Self::Entry as IntermediateTreeEntryReadWrite>::read(&mut cursor))
            .collect::<io::Result<Vec<_>>>()?;

        let size = Self::Header::HEADER_SIZE + entry_count * Self::Entry::ENTRY_SIZE;
        let offset = size + Self::Trailer::SIZE;
        let offset = i64::from(block_size(offset) - offset);
        match offset.cmp(&0) {
            Ordering::Greater => {
                f.seek(SeekFrom::Current(offset))?;
            }
            Ordering::Less => return Err(NdbError::InvalidBlockSize(size).into()),
            _ => {}
        }

        let trailer = Self::Trailer::read(f)?;
        trailer.verify_block_id(true)?;

        let crc = compute_crc(0, &data);
        if crc != trailer.crc() {
            return Err(NdbError::InvalidBlockCrc(crc).into());
        }

        Ok(Self::new(header, entries, trailer)?)
    }

    fn write<W: Write + Seek>(&self, f: &mut W) -> io::Result<()> {
        let mut curor = Cursor::new(vec![
            0_u8;
            Self::Header::HEADER_SIZE as usize
                + self.entries().len()
                    * Self::Entry::ENTRY_SIZE as usize
        ]);

        self.header().write(&mut curor)?;
        for entry in self.entries() {
            entry.write(&mut curor)?;
        }

        let data = curor.into_inner();
        let trailer = self.trailer();
        let crc = compute_crc(0, &data);
        let trailer = Self::Trailer::new(
            data.len() as u16,
            trailer.signature(),
            crc,
            trailer.block_id(),
        )?;

        let offset = trailer.size() + Self::Trailer::SIZE;
        let offset = block_size(offset) - offset;

        f.write_all(&data)?;
        f.seek(SeekFrom::Current(i64::from(offset)))?;
        trailer.write(f)
    }
}
