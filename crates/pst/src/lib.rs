#![doc = include_str!("../README.md")]

use std::{
    fs::File,
    io::{self, Seek, SeekFrom},
    path::Path,
    sync::Mutex,
};

pub mod ltp;
pub mod messaging;
pub mod ndb;

mod block_sig;
mod crc;
mod encode;

use ndb::{block::*, block_ref::*, header::*, page::*};

/// [PST File](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/6b57253b-0853-47bb-99bb-d4b8f78105f0)
pub trait PstFile {
    type Header: Header;
    type PageTrailer: PageTrailer;
    type BlockRef: BlockRef;
    type NodeBTree: RootBTree;
    type BlockBTree: RootBTree;
    type IntermediateDataTreeEntry: IntermediateTreeEntry;
    type BlockTrailer: BlockTrailer;

    fn file(&self) -> &Mutex<File>;
    fn header(&self) -> &Self::Header;
    fn density_list(&self)
        -> Result<&dyn DensityListPage<Trailer = Self::PageTrailer>, &io::Error>;
}

pub struct UnicodePstFile {
    file: Mutex<File>,
    header: UnicodeHeader,
    density_list: io::Result<UnicodeDensityListPage>,
}

impl UnicodePstFile {
    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        use ndb::read_write::{DensityListPageReadWrite, HeaderReadWrite};

        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(0))?;
        let header = UnicodeHeader::read(&mut file)?;
        let density_list = UnicodeDensityListPage::read(&mut file);
        Ok(Self {
            file: Mutex::new(file),
            header,
            density_list,
        })
    }
}

impl PstFile for UnicodePstFile {
    type Header = UnicodeHeader;
    type PageTrailer = UnicodePageTrailer;
    type BlockRef = UnicodeBlockRef;
    type NodeBTree = UnicodeNodeBTree;
    type BlockBTree = UnicodeBlockBTree;
    type IntermediateDataTreeEntry = UnicodeDataTreeEntry;
    type BlockTrailer = UnicodeBlockTrailer;

    fn file(&self) -> &Mutex<File> {
        &self.file
    }

    fn header(&self) -> &Self::Header {
        &self.header
    }

    fn density_list(
        &self,
    ) -> Result<&dyn DensityListPage<Trailer = Self::PageTrailer>, &io::Error> {
        self.density_list.as_ref().map(|dl| dl as _)
    }
}

pub struct AnsiPstFile {
    file: Mutex<File>,
    header: ndb::header::AnsiHeader,
    density_list: io::Result<ndb::page::AnsiDensityListPage>,
}

impl AnsiPstFile {
    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        use ndb::read_write::{DensityListPageReadWrite, HeaderReadWrite};

        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(0))?;
        let header = AnsiHeader::read(&mut file)?;
        let density_list = AnsiDensityListPage::read(&mut file);
        Ok(Self {
            file: Mutex::new(file),
            header,
            density_list,
        })
    }
}

impl PstFile for AnsiPstFile {
    type Header = AnsiHeader;
    type PageTrailer = AnsiPageTrailer;
    type BlockRef = AnsiBlockRef;
    type NodeBTree = AnsiNodeBTree;
    type BlockBTree = AnsiBlockBTree;
    type IntermediateDataTreeEntry = AnsiDataTreeEntry;
    type BlockTrailer = AnsiBlockTrailer;

    fn file(&self) -> &Mutex<File> {
        &self.file
    }

    fn header(&self) -> &Self::Header {
        &self.header
    }

    fn density_list(
        &self,
    ) -> Result<&dyn DensityListPage<Trailer = Self::PageTrailer>, &io::Error> {
        self.density_list.as_ref().map(|dl| dl as _)
    }
}
