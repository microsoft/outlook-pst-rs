#![doc = include_str!("../README.md")]

use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    mem,
    path::Path,
    sync::Mutex,
};
use thiserror::Error;

pub mod ltp;
pub mod messaging;
pub mod ndb;

mod block_sig;
mod crc;
mod encode;

use ndb::{
    block::*, block_id::*, block_ref::*, byte_index::*, header::*, page::*, read_write::*, root::*,
    *,
};

#[derive(Error, Debug)]
pub enum PstError {
    #[error("Cannot write to file: {0}")]
    NoWriteAccess(String),
    #[error("I/O error: {0:?}")]
    Io(#[from] io::Error),
    #[error("Failed to lock file")]
    LockError,
    #[error("Integer conversion failed")]
    IntegerConversion,
    #[error("Node Database error: {0}")]
    NodeDatabaseError(#[from] NdbError),
    #[error("AllocationMapPage not found: {0}")]
    AllocationMapPageNotFound(usize),
}

impl From<&PstError> for io::Error {
    fn from(err: &PstError) -> io::Error {
        match err {
            PstError::NoWriteAccess(path) => {
                io::Error::new(io::ErrorKind::PermissionDenied, path.as_str())
            }
            err => io::Error::other(format!("{err:?}")),
        }
    }
}

impl From<PstError> for io::Error {
    fn from(err: PstError) -> io::Error {
        match err {
            PstError::NoWriteAccess(path) => {
                io::Error::new(io::ErrorKind::PermissionDenied, path.as_str())
            }
            PstError::Io(err) => err,
            err => io::Error::other(err),
        }
    }
}

type PstResult<T> = std::result::Result<T, PstError>;

/// [PST File](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/6b57253b-0853-47bb-99bb-d4b8f78105f0)
pub trait PstFile: Sized
where
    u64: From<<Self::BlockId as BlockId>::Index> + From<<Self::ByteIndex as ByteIndex>::Index>,
{
    type BlockId: BlockId + BlockIdReadWrite;
    type ByteIndex: ByteIndex + ByteIndexReadWrite;
    type BlockRef: BlockRef<Block = Self::BlockId, Index = Self::ByteIndex> + BlockRefReadWrite;
    type Root: Root<Self>;
    type Header: Header<Self>;
    type PageTrailer: PageTrailer<BlockId = Self::BlockId> + PageTrailerReadWrite;
    type BTreeKey: BTreeEntryKey;
    type NodeBTreeEntry: NodeBTreeEntry<Block = Self::BlockId> + BTreeEntry<Key = Self::BTreeKey>;
    type NodeBTree: NodeBTree<Self, Self::NodeBTreeEntry>;
    type BlockBTreeEntry: BlockBTreeEntry<Block = Self::BlockRef> + BTreeEntry<Key = Self::BTreeKey>;
    type BlockBTree: BlockBTree<Self, Self::BlockBTreeEntry>;
    type IntermediateDataTreeEntry: IntermediateTreeEntry;
    type BlockTrailer: BlockTrailer<BlockId = Self::BlockId>;
    type AllocationMapPage: AllocationMapPage<Self>;
    type AllocationPageMapPage: AllocationPageMapPage<Self>;
    type FreeMapPage: FreeMapPage<Self>;
    type FreePageMapPage: FreePageMapPage<Self>;
    type DensityListPage: DensityListPage<Self>;

    fn reader(&self) -> &Mutex<BufReader<File>>;
    fn writer(&mut self) -> &PstResult<Mutex<BufWriter<File>>>;
    fn header(&self) -> &Self::Header;
    fn header_mut(&mut self) -> &mut Self::Header;
    fn density_list(&self) -> Result<&dyn DensityListPage<Self>, &io::Error>;

    fn start_write(&mut self) -> io::Result<()>;
    fn finish_write(&mut self) -> io::Result<()>;
}

pub struct UnicodePstFile {
    reader: Mutex<BufReader<File>>,
    writer: PstResult<Mutex<BufWriter<File>>>,
    header: UnicodeHeader,
    density_list: io::Result<UnicodeDensityListPage>,
}

impl UnicodePstFile {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let writer = OpenOptions::new()
            .write(true)
            .open(&path)
            .map(BufWriter::new)
            .map(Mutex::new)
            .map_err(|_| PstError::NoWriteAccess(path.as_ref().display().to_string()));
        let mut reader = BufReader::new(File::open(path)?);
        reader.seek(SeekFrom::Start(0))?;
        let header = UnicodeHeader::read(&mut reader)?;
        let density_list = UnicodeDensityListPage::read(&mut reader);

        Ok(Self {
            reader: Mutex::new(reader),
            writer,
            header,
            density_list,
        })
    }
}

impl PstFile for UnicodePstFile {
    type BlockId = UnicodeBlockId;
    type ByteIndex = UnicodeByteIndex;
    type BlockRef = UnicodeBlockRef;
    type Root = UnicodeRoot;
    type Header = UnicodeHeader;
    type PageTrailer = UnicodePageTrailer;
    type BTreeKey = u64;
    type NodeBTreeEntry = UnicodeNodeBTreeEntry;
    type NodeBTree = UnicodeNodeBTree;
    type BlockBTreeEntry = UnicodeBlockBTreeEntry;
    type BlockBTree = UnicodeBlockBTree;
    type IntermediateDataTreeEntry = UnicodeDataTreeEntry;
    type BlockTrailer = UnicodeBlockTrailer;
    type AllocationMapPage = UnicodeMapPage<{ PageType::AllocationMap as u8 }>;
    type AllocationPageMapPage = UnicodeMapPage<{ PageType::AllocationPageMap as u8 }>;
    type FreeMapPage = UnicodeMapPage<{ PageType::FreeMap as u8 }>;
    type FreePageMapPage = UnicodeMapPage<{ PageType::FreePageMap as u8 }>;
    type DensityListPage = UnicodeDensityListPage;

    fn reader(&self) -> &Mutex<BufReader<File>> {
        &self.reader
    }

    fn writer(&mut self) -> &PstResult<Mutex<BufWriter<File>>> {
        &self.writer
    }

    fn header(&self) -> &Self::Header {
        &self.header
    }

    fn header_mut(&mut self) -> &mut Self::Header {
        &mut self.header
    }

    fn density_list(&self) -> Result<&dyn DensityListPage<Self>, &io::Error> {
        self.density_list.as_ref().map(|dl| dl as _)
    }

    fn start_write(&mut self) -> io::Result<()> {
        <Self as PstFileReadWrite>::start_write(self)
    }

    fn finish_write(&mut self) -> io::Result<()> {
        <Self as PstFileReadWrite>::finish_write(self)
    }
}

pub struct AnsiPstFile {
    reader: Mutex<BufReader<File>>,
    writer: PstResult<Mutex<BufWriter<File>>>,
    header: ndb::header::AnsiHeader,
    density_list: io::Result<ndb::page::AnsiDensityListPage>,
}

impl AnsiPstFile {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let writer = OpenOptions::new()
            .write(true)
            .open(&path)
            .map(BufWriter::new)
            .map(Mutex::new)
            .map_err(|_| PstError::NoWriteAccess(path.as_ref().display().to_string()));
        let mut reader = BufReader::new(File::open(path)?);
        let header = AnsiHeader::read(&mut reader)?;
        let density_list = AnsiDensityListPage::read(&mut reader);
        Ok(Self {
            reader: Mutex::new(reader),
            writer,
            header,
            density_list,
        })
    }
}

impl PstFile for AnsiPstFile {
    type BlockId = AnsiBlockId;
    type ByteIndex = AnsiByteIndex;
    type BlockRef = AnsiBlockRef;
    type Root = AnsiRoot;
    type Header = AnsiHeader;
    type PageTrailer = AnsiPageTrailer;
    type BTreeKey = u32;
    type NodeBTreeEntry = AnsiNodeBTreeEntry;
    type NodeBTree = AnsiNodeBTree;
    type BlockBTreeEntry = AnsiBlockBTreeEntry;
    type BlockBTree = AnsiBlockBTree;
    type IntermediateDataTreeEntry = AnsiDataTreeEntry;
    type BlockTrailer = AnsiBlockTrailer;
    type AllocationMapPage = AnsiMapPage<{ PageType::AllocationMap as u8 }>;
    type AllocationPageMapPage = AnsiMapPage<{ PageType::AllocationPageMap as u8 }>;
    type FreeMapPage = AnsiMapPage<{ PageType::FreeMap as u8 }>;
    type FreePageMapPage = AnsiMapPage<{ PageType::FreePageMap as u8 }>;
    type DensityListPage = AnsiDensityListPage;

    fn reader(&self) -> &Mutex<BufReader<File>> {
        &self.reader
    }

    fn writer(&mut self) -> &PstResult<Mutex<BufWriter<File>>> {
        &self.writer
    }

    fn header(&self) -> &Self::Header {
        &self.header
    }

    fn header_mut(&mut self) -> &mut Self::Header {
        &mut self.header
    }

    fn density_list(&self) -> Result<&dyn DensityListPage<Self>, &io::Error> {
        self.density_list.as_ref().map(|dl| dl as _)
    }

    fn start_write(&mut self) -> io::Result<()> {
        <Self as PstFileReadWrite>::start_write(self)
    }

    fn finish_write(&mut self) -> io::Result<()> {
        <Self as PstFileReadWrite>::finish_write(self)
    }
}

const AMAP_FIRST_OFFSET: u64 = 0x4400;
const AMAP_DATA_SIZE: u64 = size_of::<MapBits>() as u64 * 8 * 64;

const PMAP_FIRST_OFFSET: u64 = AMAP_FIRST_OFFSET + PAGE_SIZE as u64;
const PMAP_PAGE_COUNT: u64 = 8;
const PMAP_DATA_SIZE: u64 = AMAP_DATA_SIZE * PMAP_PAGE_COUNT;

const FMAP_FIRST_SIZE: u64 = 128;
const FMAP_FIRST_DATA_SIZE: u64 = AMAP_DATA_SIZE * FMAP_FIRST_SIZE;
const FMAP_FIRST_OFFSET: u64 = AMAP_FIRST_OFFSET + FMAP_FIRST_DATA_SIZE + (2 * PAGE_SIZE) as u64;
const FMAP_PAGE_COUNT: u64 = size_of::<MapBits>() as u64;
const FMAP_DATA_SIZE: u64 = AMAP_DATA_SIZE * FMAP_PAGE_COUNT;

const FPMAP_FIRST_SIZE: u64 = 128 * 64;
const FPMAP_FIRST_DATA_SIZE: u64 = AMAP_DATA_SIZE * FPMAP_FIRST_SIZE;
const FPMAP_FIRST_OFFSET: u64 = AMAP_FIRST_OFFSET + FPMAP_FIRST_DATA_SIZE + (3 * PAGE_SIZE) as u64;
const FPMAP_PAGE_COUNT: u64 = size_of::<MapBits>() as u64 * 64;
const FPMAP_DATA_SIZE: u64 = AMAP_DATA_SIZE * FPMAP_PAGE_COUNT;

struct AllocationMapPageInfo<Pst>
where
    Pst: PstFile,
    <Pst as PstFile>::AllocationMapPage: AllocationMapPageReadWrite<Pst>,
    u64: From<<<Pst as PstFile>::BlockId as BlockId>::Index>
        + From<<<Pst as PstFile>::ByteIndex as ByteIndex>::Index>,
{
    amap_page: <Pst as PstFile>::AllocationMapPage,
    free_space: u64,
}

impl<Pst> AllocationMapPageInfo<Pst>
where
    Pst: PstFile,
    <Pst as PstFile>::AllocationMapPage: AllocationMapPageReadWrite<Pst>,
    u64: From<<<Pst as PstFile>::BlockId as BlockId>::Index>
        + From<<<Pst as PstFile>::ByteIndex as ByteIndex>::Index>,
{
    fn max_free_slots(&self) -> u8 {
        u8::try_from(self.amap_page.find_free_bits(0xFF).len()).unwrap_or(0xFF)
    }
}

type PstFileReadWriteNodeBTree<Pst> = RootBTreePage<
    Pst,
    <<Pst as PstFile>::NodeBTree as RootBTree>::Entry,
    <<Pst as PstFile>::NodeBTree as RootBTree>::IntermediatePage,
    <<Pst as PstFile>::NodeBTree as RootBTree>::LeafPage,
>;

type PstFileReadWriteBlockBTree<Pst> = RootBTreePage<
    Pst,
    <<Pst as PstFile>::BlockBTree as RootBTree>::Entry,
    <<Pst as PstFile>::BlockBTree as RootBTree>::IntermediatePage,
    <<Pst as PstFile>::BlockBTree as RootBTree>::LeafPage,
>;

trait PstFileReadWrite: PstFile
where
    <Self as PstFile>::BlockId:
        From<<<Self as PstFile>::ByteIndex as ByteIndex>::Index> + BlockIdReadWrite,
    <Self as PstFile>::ByteIndex: ByteIndex<Index: TryFrom<u64>> + ByteIndexReadWrite,
    <Self as PstFile>::BlockRef: BlockRefReadWrite,
    <Self as PstFile>::Root: RootReadWrite<Self>,
    <Self as PstFile>::Header: HeaderReadWrite<Self>,
    <Self as PstFile>::PageTrailer: PageTrailerReadWrite,
    <Self as PstFile>::BTreeKey: BTreePageKeyReadWrite,
    <Self as PstFile>::NodeBTreeEntry: NodeBTreeEntryReadWrite,
    <Self as PstFile>::NodeBTree: RootBTreeReadWrite,
    <<Self as PstFile>::NodeBTree as RootBTree>::IntermediatePage:
        RootBTreeIntermediatePageReadWrite<
            Self,
            <Self as PstFile>::NodeBTreeEntry,
            <<Self as PstFile>::NodeBTree as RootBTree>::LeafPage,
        >,
    <<Self as PstFile>::NodeBTree as RootBTree>::LeafPage: RootBTreeLeafPageReadWrite<Self>,
    <Self as PstFile>::BlockBTreeEntry: BlockBTreeEntryReadWrite,
    <Self as PstFile>::BlockBTree: RootBTreeReadWrite,

    <<Self as PstFile>::BlockBTree as RootBTree>::IntermediatePage:
        RootBTreeIntermediatePageReadWrite<
            Self,
            <Self as PstFile>::BlockBTreeEntry,
            <<Self as PstFile>::BlockBTree as RootBTree>::LeafPage,
        >,
    <<Self as PstFile>::BlockBTree as RootBTree>::LeafPage: RootBTreeLeafPageReadWrite<Self>,
    <Self as PstFile>::BlockTrailer: BlockTrailerReadWrite,
    <Self as PstFile>::AllocationMapPage: AllocationMapPageReadWrite<Self>,
    <Self as PstFile>::AllocationPageMapPage: AllocationPageMapPageReadWrite<Self>,
    <Self as PstFile>::FreeMapPage: FreeMapPageReadWrite<Self>,
    <Self as PstFile>::FreePageMapPage: FreePageMapPageReadWrite<Self>,
    <Self as PstFile>::DensityListPage: DensityListPageReadWrite<Self>,
    u64: From<<<Self as PstFile>::BlockId as BlockId>::Index>
        + From<<<Self as PstFile>::ByteIndex as ByteIndex>::Index>,
{
    fn start_write(&mut self) -> io::Result<()> {
        self.rebuild_allocation_map()?;

        let header = {
            let header = self.header_mut();
            header.update_unique();

            let root = header.root_mut();
            root.set_amap_status(AmapStatus::Invalid);
            header.clone()
        };

        let mut writer = self
            .writer()
            .as_ref()?
            .lock()
            .map_err(|_| PstError::LockError)?;
        let writer = &mut *writer;
        writer.seek(SeekFrom::Start(0))?;
        header.write(writer)?;
        writer.flush()
    }

    fn finish_write(&mut self) -> io::Result<()> {
        let header = {
            let header = self.header_mut();
            header.update_unique();
            let root = header.root_mut();
            root.set_amap_status(AmapStatus::Valid2);
            header.clone()
        };

        let mut writer = self
            .writer()
            .as_ref()?
            .lock()
            .map_err(|_| PstError::LockError)?;
        let writer = &mut *writer;
        writer.seek(SeekFrom::Start(0))?;
        header.write(writer)?;
        writer.flush()
    }

    /// [Crash Recovery and AMap Rebuilding](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/d9bcc1fd-c66a-41b3-b6d7-ed09d2a25ced)
    fn rebuild_allocation_map(&mut self) -> io::Result<()> {
        let header = self.header();
        let root = header.root();
        if AmapStatus::Invalid != root.amap_is_valid() {
            return Ok(());
        }

        let num_amap_pages = u64::from(root.file_eof_index().index()) - AMAP_FIRST_OFFSET;
        let num_amap_pages = num_amap_pages.div_ceil(AMAP_DATA_SIZE);

        let mut amap_pages: Vec<_> = (0..num_amap_pages)
            .map(|index| {
                let has_pmap_page = index % 8 == 0;
                let has_fmap_page = has_pmap_page
                    && index >= FMAP_FIRST_SIZE
                    && (index - FMAP_FIRST_SIZE) % FMAP_PAGE_COUNT == 0;
                let has_fpmap_page = has_pmap_page
                    && index >= FPMAP_FIRST_SIZE
                    && (index - FPMAP_FIRST_SIZE) % FPMAP_PAGE_COUNT == 0;

                let index =
                    <<<Self as PstFile>::ByteIndex as ByteIndex>::Index as TryFrom<u64>>::try_from(
                        index * AMAP_DATA_SIZE + AMAP_FIRST_OFFSET,
                    )
                    .map_err(|_| PstError::IntegerConversion)?;
                let index = <Self as PstFile>::BlockId::from(index);

                let trailer = <<Self as PstFile>::PageTrailer as PageTrailerReadWrite>::new(
                    PageType::AllocationMap,
                    0,
                    index,
                    0,
                );

                let mut map_bits = [0; mem::size_of::<MapBits>()];
                let mut reserved = 1;
                if has_pmap_page {
                    reserved += 1;
                }
                if has_fmap_page {
                    reserved += 1;
                }
                if has_fpmap_page {
                    reserved += 1;
                }

                let free_space = AMAP_DATA_SIZE - (reserved * PAGE_SIZE) as u64;

                let reserved = &[0xFF; 4][..reserved];
                map_bits[..reserved.len()].copy_from_slice(reserved);

                let amap_page =
                    <<Self as PstFile>::AllocationMapPage as AllocationMapPageReadWrite<Self>>::new(
                        map_bits, trailer,
                    )?;
                Ok(AllocationMapPageInfo::<Self> {
                    amap_page,
                    free_space,
                })
            })
            .collect::<PstResult<Vec<_>>>()?;

        {
            let mut reader = self.reader().lock().map_err(|_| PstError::LockError)?;
            let reader = &mut *reader;

            let node_btree =
                <Self::NodeBTree as RootBTreeReadWrite>::read(reader, *root.node_btree())?;

            self.mark_node_btree_allocations(
                reader,
                root.node_btree().index(),
                &node_btree,
                &mut amap_pages,
            )?;

            let block_btree =
                <Self::BlockBTree as RootBTreeReadWrite>::read(reader, *root.block_btree())?;

            self.mark_block_btree_allocations(
                reader,
                root.block_btree().index(),
                &block_btree,
                &mut amap_pages,
            )?;
        }

        let free_bytes = amap_pages.iter().map(|page| page.free_space).sum();

        let mut first_fmap = [0; FMAP_FIRST_SIZE as usize];
        for (entry, free_space) in first_fmap
            .iter_mut()
            .zip(amap_pages.iter().map(|page| page.max_free_slots()))
        {
            *entry = free_space;
        }

        let pmap_pages: Vec<_> = (0..=(num_amap_pages / 8))
            .map(|index| {
                let index =
                    <<<Self as PstFile>::ByteIndex as ByteIndex>::Index as TryFrom<u64>>::try_from(
                        index * PMAP_DATA_SIZE + PMAP_FIRST_OFFSET,
                    )
                    .map_err(|_| PstError::IntegerConversion)?;
                let index = <Self as PstFile>::BlockId::from(index);

                let trailer = <<Self as PstFile>::PageTrailer as PageTrailerReadWrite>::new(
                    PageType::AllocationPageMap,
                    0,
                    index,
                    0,
                );

                let map_bits = [0xFF; mem::size_of::<MapBits>()];

                let pmap_page =
                    <<Self as PstFile>::AllocationPageMapPage as AllocationPageMapPageReadWrite<
                        Self,
                    >>::new(map_bits, trailer)?;
                Ok(pmap_page)
            })
            .collect::<PstResult<Vec<_>>>()?;

        let fmap_pages: Vec<_> = (0..(num_amap_pages.max(FMAP_FIRST_SIZE) - FMAP_FIRST_SIZE)
            .div_ceil(FMAP_PAGE_COUNT))
            .map(|index| {
                let amap_index =
                    FMAP_FIRST_SIZE as usize + (index as usize * mem::size_of::<MapBits>());
                let index =
                    <<<Self as PstFile>::ByteIndex as ByteIndex>::Index as TryFrom<u64>>::try_from(
                        index * FMAP_DATA_SIZE + FMAP_FIRST_OFFSET,
                    )
                    .map_err(|_| PstError::IntegerConversion)?;
                let index = <Self as PstFile>::BlockId::from(index);

                let trailer = <<Self as PstFile>::PageTrailer as PageTrailerReadWrite>::new(
                    PageType::FreeMap,
                    0,
                    index,
                    0,
                );

                let mut map_bits = [0; mem::size_of::<MapBits>()];
                for (entry, free_space) in map_bits.iter_mut().zip(
                    amap_pages
                        .iter()
                        .skip(amap_index)
                        .map(|page| page.max_free_slots()),
                ) {
                    *entry = free_space;
                }

                let fmap_page =
                    <<Self as PstFile>::FreeMapPage as FreeMapPageReadWrite<Self>>::new(
                        map_bits, trailer,
                    )?;
                Ok(fmap_page)
            })
            .collect::<PstResult<Vec<_>>>()?;

        let fpmap_pages: Vec<_> = (0..(num_amap_pages.max(FPMAP_FIRST_SIZE) - FPMAP_FIRST_SIZE)
            .div_ceil(FPMAP_PAGE_COUNT))
            .map(|index| {
                let index =
                    <<<Self as PstFile>::ByteIndex as ByteIndex>::Index as TryFrom<u64>>::try_from(
                        index * FPMAP_DATA_SIZE + FPMAP_FIRST_OFFSET,
                    )
                    .map_err(|_| PstError::IntegerConversion)?;
                let index = <Self as PstFile>::BlockId::from(index);

                let trailer = <<Self as PstFile>::PageTrailer as PageTrailerReadWrite>::new(
                    PageType::FreePageMap,
                    0,
                    index,
                    0,
                );

                let map_bits = [0xFF; mem::size_of::<MapBits>()];

                let fmap_page = <<Self as PstFile>::FreePageMapPage as FreePageMapPageReadWrite<
                    Self,
                >>::new(map_bits, trailer)?;
                Ok(fmap_page)
            })
            .collect::<PstResult<Vec<_>>>()?;

        {
            let mut writer = self
                .writer()
                .as_ref()?
                .lock()
                .map_err(|_| PstError::LockError)?;
            let writer = &mut *writer;

            for page in amap_pages.into_iter().map(|info| info.amap_page) {
                let index: <<Self as PstFile>::BlockId as BlockId>::Index =
                    page.trailer().block_id().into();
                let index = u64::from(index);

                writer.seek(SeekFrom::Start(index))?;
                <Self::AllocationMapPage as AllocationMapPageReadWrite<Self>>::write(
                    &page, writer,
                )?;
            }

            for page in pmap_pages.into_iter() {
                let index: <<Self as PstFile>::BlockId as BlockId>::Index =
                    page.trailer().block_id().into();
                let index = u64::from(index);

                writer.seek(SeekFrom::Start(index))?;
                <Self::AllocationPageMapPage as AllocationPageMapPageReadWrite<Self>>::write(
                    &page, writer,
                )?;
            }

            for page in fmap_pages.into_iter() {
                let index: <<Self as PstFile>::BlockId as BlockId>::Index =
                    page.trailer().block_id().into();
                let index = u64::from(index);

                writer.seek(SeekFrom::Start(index))?;
                <Self::FreeMapPage as FreeMapPageReadWrite<Self>>::write(&page, writer)?;
            }

            for page in fpmap_pages.into_iter() {
                let index: <<Self as PstFile>::BlockId as BlockId>::Index =
                    page.trailer().block_id().into();
                let index = u64::from(index);

                writer.seek(SeekFrom::Start(index))?;
                <Self::FreePageMapPage as FreePageMapPageReadWrite<Self>>::write(&page, writer)?;
            }

            writer.flush()?;
        }

        let header = self.header_mut();
        <<Self as PstFile>::Header as HeaderReadWrite<Self>>::first_free_map(header)
            .copy_from_slice(&first_fmap);
        let root = header.root_mut();
        root.reset_free_size(free_bytes)?;
        root.set_amap_status(AmapStatus::Valid2);

        Ok(())
    }

    fn mark_node_btree_allocations<R: Read + Seek>(
        &self,
        reader: &mut R,
        page_index: Self::ByteIndex,
        node_btree: &PstFileReadWriteNodeBTree<Self>,
        amap_pages: &mut Vec<AllocationMapPageInfo<Self>>,
    ) -> io::Result<()> {
        Self::mark_page_allocation(u64::from(page_index.index()), amap_pages)?;

        if let RootBTreePage::Intermediate(page, ..) = node_btree {
            for entry in page.entries() {
                let block = entry.block();
                let node_btree = <Self::NodeBTree as RootBTreeReadWrite>::read(reader, block)?;
                self.mark_node_btree_allocations(reader, block.index(), &node_btree, amap_pages)?;
            }
        }

        Ok(())
    }

    fn mark_block_btree_allocations<R: Read + Seek>(
        &self,
        reader: &mut R,
        page_index: Self::ByteIndex,
        block_btree: &PstFileReadWriteBlockBTree<Self>,
        amap_pages: &mut Vec<AllocationMapPageInfo<Self>>,
    ) -> io::Result<()> {
        Self::mark_page_allocation(u64::from(page_index.index()), amap_pages)?;

        match block_btree {
            RootBTreePage::Intermediate(page, ..) => {
                for entry in page.entries() {
                    let block_btree =
                        <Self::BlockBTree as RootBTreeReadWrite>::read(reader, entry.block())?;
                    self.mark_block_btree_allocations(
                        reader,
                        entry.block().index(),
                        &block_btree,
                        amap_pages,
                    )?;
                }
            }
            RootBTreePage::Leaf(page) => {
                for entry in page.entries() {
                    Self::mark_block_allocation(
                        u64::from(entry.block().index().index()),
                        entry.size(),
                        amap_pages,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn mark_page_allocation(
        index: u64,
        amap_pages: &mut Vec<AllocationMapPageInfo<Self>>,
    ) -> io::Result<()> {
        let index = index - AMAP_FIRST_OFFSET;
        let amap_index =
            usize::try_from(index / AMAP_DATA_SIZE).map_err(|_| PstError::IntegerConversion)?;
        let entry = amap_pages
            .get_mut(amap_index)
            .ok_or(PstError::AllocationMapPageNotFound(amap_index))?;
        entry.free_space -= PAGE_SIZE as u64;

        let bytes = entry.amap_page.map_bits_mut();

        let bit_index = usize::try_from((index % AMAP_DATA_SIZE) / 64)
            .map_err(|_| PstError::IntegerConversion)?;
        let byte_index = bit_index / 8;
        let bit_index = bit_index % 8;

        if bit_index == 0 {
            bytes[byte_index] = 0xFF;
        } else {
            let mask = 0x80_u8 >> bit_index;
            let mask = mask | (mask - 1);
            bytes[byte_index] |= mask;
            bytes[byte_index + 1] |= !mask;
        }

        Ok(())
    }

    fn mark_block_allocation(
        index: u64,
        size: u16,
        amap_pages: &mut Vec<AllocationMapPageInfo<Self>>,
    ) -> io::Result<()> {
        let index = index - AMAP_FIRST_OFFSET;
        let amap_index =
            usize::try_from(index / AMAP_DATA_SIZE).map_err(|_| PstError::IntegerConversion)?;
        let entry = amap_pages
            .get_mut(amap_index)
            .ok_or(PstError::AllocationMapPageNotFound(amap_index))?;
        let size = u64::from(block_size(
            size + <<Self as PstFile>::BlockTrailer as BlockTrailerReadWrite>::SIZE,
        ));
        entry.free_space -= size;

        let bytes = entry.amap_page.map_bits_mut();

        let bit_start = usize::try_from((index % AMAP_DATA_SIZE) / 64)
            .map_err(|_| PstError::IntegerConversion)?;
        let bit_end =
            bit_start + usize::try_from(size / 64).map_err(|_| PstError::IntegerConversion)?;
        let byte_start = bit_start / 8;
        let bit_start = bit_start % 8;
        let byte_end = bit_end / 8;
        let bit_end = bit_end % 8;

        if byte_start == byte_end {
            // The allocation fits in a single byte
            if bit_end > bit_start {
                let mask_start = 0x80_u8 >> bit_start;
                let mask_start = mask_start | (mask_start - 1);
                let mask_end = 0x80_u8 >> bit_end;
                let mask_end = !(mask_end | (mask_end - 1));
                let mask = mask_start & mask_end;
                bytes[byte_start] |= mask;
            }
            return Ok(());
        }

        let byte_start = if bit_start == 0 {
            byte_start
        } else {
            let mask_start = 0x80_u8 >> bit_start;
            let mask_start = mask_start | (mask_start - 1);
            bytes[byte_start] |= mask_start;
            byte_start + 1
        };

        if bit_end != 0 {
            let mask_end = 0x80_u8 >> bit_end;
            let mask_end = !(mask_end | (mask_end - 1));
            bytes[byte_end] |= mask_end;
        };

        if byte_end > byte_start {
            for byte in &mut bytes[byte_start..byte_end] {
                *byte = 0xFF;
            }
        }

        Ok(())
    }
}

impl PstFileReadWrite for UnicodePstFile {}
impl PstFileReadWrite for AnsiPstFile {}
