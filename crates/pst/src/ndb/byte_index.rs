//! [IB (Byte Index)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/7d53d413-b492-4483-b624-4e2fa2a08cf3)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::{
    fmt::Debug,
    io::{self, Read, Write},
};

use super::read_write::*;

pub trait ByteIndex: Copy + Default + Sized {
    type Index: Copy + Sized + Into<u64>;

    fn index(&self) -> Self::Index;
}

#[derive(Clone, Copy, Default)]
pub struct UnicodeByteIndex(u64);

impl UnicodeByteIndex {
    pub fn new(index: u64) -> Self {
        Self(index)
    }
}

impl ByteIndex for UnicodeByteIndex {
    type Index = u64;

    fn index(&self) -> u64 {
        self.0
    }
}

impl ByteIndexReadWrite for UnicodeByteIndex {
    fn new(index: u64) -> Self {
        Self::new(index)
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u64::<LittleEndian>()?;
        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u64::<LittleEndian>(self.0)
    }
}

impl Debug for UnicodeByteIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnicodeByteIndex {{ 0x{:X} }}", self.index())
    }
}

impl From<u64> for UnicodeByteIndex {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<UnicodeByteIndex> for u64 {
    fn from(value: UnicodeByteIndex) -> Self {
        value.index()
    }
}

#[derive(Clone, Copy, Default)]
pub struct AnsiByteIndex(u32);

impl AnsiByteIndex {
    pub fn new(index: u32) -> Self {
        Self(index)
    }
}

impl ByteIndex for AnsiByteIndex {
    type Index = u32;

    fn index(&self) -> u32 {
        self.0
    }
}

impl ByteIndexReadWrite for AnsiByteIndex {
    fn new(index: u32) -> Self {
        Self::new(index)
    }

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let value = f.read_u32::<LittleEndian>()?;
        Ok(Self(value))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.0)
    }
}

impl Debug for AnsiByteIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AnsiByteIndex {{ 0x{:X} }}", self.index())
    }
}

impl From<u32> for AnsiByteIndex {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<AnsiByteIndex> for u32 {
    fn from(value: AnsiByteIndex) -> Self {
        value.index()
    }
}
