#![doc = include_str!("../README.md")]

use std::{fs::File, io::{self, Seek, SeekFrom}, path::Path, sync::Mutex};

pub mod ltp;
pub mod messaging;
pub mod ndb;

mod crc;

/// [PST File](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/6b57253b-0853-47bb-99bb-d4b8f78105f0)
pub struct PstFile {
    file: Mutex<File>,
    header: ndb::UnicodeHeader,
}

impl PstFile {
    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(0))?;
        let header = ndb::UnicodeHeader::read(&mut file)?;
        Ok(Self {
            file: Mutex::new(file),
            header,
        })
    }

    pub fn file(&self) -> &Mutex<File> {
        &self.file
    }

    pub fn header(&self) -> &ndb::UnicodeHeader {
        &self.header
    }
}
