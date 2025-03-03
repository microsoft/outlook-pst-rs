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

/// [PST File](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/6b57253b-0853-47bb-99bb-d4b8f78105f0)
pub struct UnicodePstFile {
    file: Mutex<File>,
    header: ndb::header::UnicodeHeader,
    density_list: io::Result<ndb::page::UnicodeDensityListPage>,
}

impl UnicodePstFile {
    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        use ndb::read_write::{DensityListPageReadWrite, HeaderReadWrite};

        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(0))?;
        let header = ndb::header::UnicodeHeader::read(&mut file)?;
        let density_list = ndb::page::UnicodeDensityListPage::read(&mut file);
        Ok(Self {
            file: Mutex::new(file),
            header,
            density_list,
        })
    }

    pub fn file(&self) -> &Mutex<File> {
        &self.file
    }

    pub fn header(&self) -> &ndb::header::UnicodeHeader {
        &self.header
    }

    pub fn density_list(&self) -> &io::Result<ndb::page::UnicodeDensityListPage> {
        &self.density_list
    }
}

/// [PST File](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/6b57253b-0853-47bb-99bb-d4b8f78105f0)
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
        let header = ndb::header::AnsiHeader::read(&mut file)?;
        let density_list = ndb::page::AnsiDensityListPage::read(&mut file);
        Ok(Self {
            file: Mutex::new(file),
            header,
            density_list,
        })
    }

    pub fn file(&self) -> &Mutex<File> {
        &self.file
    }

    pub fn header(&self) -> &ndb::header::AnsiHeader {
        &self.header
    }

    pub fn density_list(&self) -> &io::Result<ndb::page::AnsiDensityListPage> {
        &self.density_list
    }
}
