//! ## [BTree-on-Heap (BTH)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/2dd1a95a-c8b1-4ac5-87d1-10cb8de64053)

use byteorder::{ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};

use super::{heap::*, read_write::*, *};

/// [BTHHEADER](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/8e4ae05c-3c24-4103-b7e5-ffef6f244834)
#[derive(Clone, Copy, Debug)]
pub struct HeapTreeHeader {
    key_size: u8,
    entry_size: u8,
    levels: u8,
    root: HeapId,
}

impl HeapTreeHeader {
    pub fn new(key_size: u8, entry_size: u8, levels: u8, root: HeapId) -> Self {
        Self {
            key_size,
            entry_size,
            levels,
            root,
        }
    }

    pub fn key_size(&self) -> u8 {
        self.key_size
    }

    pub fn entry_size(&self) -> u8 {
        self.entry_size
    }

    pub fn levels(&self) -> u8 {
        self.levels
    }

    pub fn root(&self) -> HeapId {
        self.root
    }
}

impl HeapNodeReadWrite for HeapTreeHeader {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let heap_type = HeapNodeType::try_from(f.read_u8()?)?;
        if heap_type != HeapNodeType::Tree {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                LtpError::InvalidHeapTreeNodeType(heap_type),
            ));
        }

        let key_size = f.read_u8()?;
        let entry_size = f.read_u8()?;
        let levels = f.read_u8()?;
        let root = HeapId::read(f)?;

        Ok(Self::new(key_size, entry_size, levels, root))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u8(HeapNodeType::Tree as u8)?;
        f.write_u8(self.key_size)?;
        f.write_u8(self.entry_size)?;
        f.write_u8(self.levels)?;
        self.root.write(f)
    }
}

/// [Intermediate BTH (Index) Records](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/2c992ac1-1b21-4167-b111-f76cf609005f)
#[derive(Clone, Copy, Debug)]
pub struct HeapTreeIntermediateEntry<K>
where
    K: Copy + Sized,
{
    key: K,
    next_level: HeapId,
}

impl<K> HeapTreeIntermediateEntry<K>
where
    K: Copy + Sized,
{
    pub fn new(key: K, next_level: HeapId) -> Self {
        Self { key, next_level }
    }

    pub fn key(&self) -> K {
        self.key
    }

    pub fn next_level(&self) -> HeapId {
        self.next_level
    }
}

impl<K> HeapNodeReadWrite for HeapTreeIntermediateEntry<K>
where
    K: HeapNodeReadWrite + Copy,
{
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let key = K::read(f)?;
        let next_level = HeapId::read(f)?;

        Ok(Self::new(key, next_level))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.key.write(f)?;
        self.next_level.write(f)
    }
}

/// [Leaf BTH (Data) Records](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/660db569-c8f7-4516-82ad-44709b1c667f)
#[derive(Clone, Copy, Debug)]
pub struct HeapTreeLeafEntry<K, V>
where
    K: Copy + Sized,
    V: Copy + Sized,
{
    key: K,
    data: V,
}

impl<K, V> HeapTreeLeafEntry<K, V>
where
    K: Copy + Sized,
    V: Copy + Sized,
{
    pub fn new(key: K, data: V) -> Self {
        Self { key, data }
    }

    pub fn key(&self) -> K {
        self.key
    }

    pub fn data(&self) -> V {
        self.data
    }
}

impl<K, V> HeapNodeReadWrite for HeapTreeLeafEntry<K, V>
where
    K: HeapNodeReadWrite + Copy,
    V: HeapNodeReadWrite + Copy,
{
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let key = K::read(f)?;
        let data = V::read(f)?;

        Ok(Self::new(key, data))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.key.write(f)?;
        self.data.write(f)
    }
}
