#![doc = include_str!("../README.md")]

use std::{fs::File, io, path::Path};

pub mod ltp;
pub mod messaging;
pub mod ndb;

mod crc;

/// [PST File](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/6b57253b-0853-47bb-99bb-d4b8f78105f0)
pub struct PstFile {
    _file: File,
}

impl PstFile {
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            _file: File::open(path)?,
        })
    }
}
