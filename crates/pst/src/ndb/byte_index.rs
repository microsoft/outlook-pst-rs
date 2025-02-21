//! [IB (Byte Index)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/7d53d413-b492-4483-b624-4e2fa2a08cf3)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};

use super::read_write::*;

pub trait ByteIndex {
    type Index: Copy + Sized;

    fn index(&self) -> Self::Index;
}

#[derive(Clone, Copy, Default, Debug)]
pub struct UnicodeByteIndex(u64);

impl ByteIndex for UnicodeByteIndex {
    type Index = u64;

    fn index(&self) -> u64 {
        self.0
    }
}

impl ByteIndexReadWrite for UnicodeByteIndex {
    fn new(index: u64) -> Self {
        Self(index)
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u64::<LittleEndian>()?;
        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u64::<LittleEndian>(self.0)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct AnsiByteIndex(u32);

impl ByteIndex for AnsiByteIndex {
    type Index = u32;

    fn index(&self) -> u32 {
        self.0
    }
}

impl ByteIndexReadWrite for AnsiByteIndex {
    fn new(index: u32) -> Self {
        Self(index)
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u32::<LittleEndian>()?;
        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.0)
    }
}
