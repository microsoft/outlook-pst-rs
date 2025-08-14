//! ## [BTree-on-Heap (BTH)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/2dd1a95a-c8b1-4ac5-87d1-10cb8de64053)

use byteorder::{ReadBytesExt, WriteBytesExt};
use core::mem;
use std::{
    io::{self, Cursor, Read, Write},
    marker::PhantomData,
};

use super::{heap::*, read_write::*, *};
use crate::{AnsiPstFile, PstFile, UnicodePstFile};

/// [BTHHEADER](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/8e4ae05c-3c24-4103-b7e5-ffef6f244834)
#[derive(Clone, Copy, Debug)]
pub struct HeapTreeHeader {
    key_size: u8,
    entry_size: u8,
    levels: u8,
    root: HeapId,
}

impl HeapTreeHeader {
    pub fn new(key_size: u8, entry_size: u8, levels: u8, root: HeapId) -> LtpResult<Self> {
        match key_size {
            2 | 4 | 8 | 16 => {}
            invalid => {
                return Err(LtpError::InvalidHeapTreeKeySize(invalid));
            }
        }

        match entry_size {
            1..=32 => {}
            invalid => {
                return Err(LtpError::InvalidHeapTreeDataSize(invalid));
            }
        }

        Ok(Self {
            key_size,
            entry_size,
            levels,
            root,
        })
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

pub trait HeapTreeEntryKey: Copy + Sized + PartialEq + PartialOrd {
    const SIZE: u8;
}

pub trait HeapTreeEntryValue: Copy + Sized {
    const SIZE: u8;
}

impl HeapNodePageReadWrite for HeapTreeHeader {
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

        Ok(Self::new(key_size, entry_size, levels, root)?)
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
    K: HeapTreeEntryKey,
{
    key: K,
    next_level: HeapId,
}

impl<K> HeapTreeIntermediateEntry<K>
where
    K: HeapTreeEntryKey,
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

impl<K> HeapNodePageReadWrite for HeapTreeIntermediateEntry<K>
where
    K: HeapNodePageReadWrite + HeapTreeEntryKey,
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
    K: HeapTreeEntryKey,
    V: Copy + Sized,
{
    key: K,
    data: V,
}

impl<K, V> HeapTreeLeafEntry<K, V>
where
    K: HeapTreeEntryKey,
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

impl<K, V> HeapNodePageReadWrite for HeapTreeLeafEntry<K, V>
where
    K: HeapNodePageReadWrite + HeapTreeEntryKey,
    V: HeapNodePageReadWrite + Copy,
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

pub trait HeapTree {
    type Key: HeapTreeEntryKey;
    type Value: HeapTreeEntryValue;

    fn heap(&self) -> &dyn HeapNode;
    fn user_root(&self) -> HeapId;
    fn header(&self) -> io::Result<HeapTreeHeader>;
    fn entries(&self) -> io::Result<Vec<HeapTreeLeafEntry<Self::Key, Self::Value>>>;
}

struct HeapTreeInner<Pst, K, V>
where
    Pst: PstFile,
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    heap: <Pst as PstFile>::HeapNode,
    user_root: HeapId,
    _phantom: PhantomData<(K, V)>,
}

impl<Pst, K, V> HeapTreeInner<Pst, K, V>
where
    Pst: PstFile,
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    fn new(heap: <Pst as PstFile>::HeapNode, user_root: HeapId) -> Self {
        Self {
            heap,
            user_root,
            _phantom: PhantomData,
        }
    }

    fn header(&self) -> io::Result<HeapTreeHeader> {
        let mut cursor = Cursor::new(self.heap.find_entry(self.user_root)?);
        HeapTreeHeader::read(&mut cursor)
    }

    fn entries(&self) -> io::Result<Vec<HeapTreeLeafEntry<K, V>>> {
        let header = self.header()?;
        if header.key_size() != K::SIZE {
            return Err(LtpError::InvalidHeapTreeKeySize(header.key_size()).into());
        }
        if header.entry_size() != V::SIZE {
            return Err(LtpError::InvalidHeapTreeDataSize(header.entry_size()).into());
        }

        if u32::from(header.root()) == 0 {
            return Ok(Default::default());
        }

        let mut level = header.levels();
        let mut next_level = vec![header.root()];

        while level > 0 {
            for heap_id in mem::take(&mut next_level).into_iter() {
                let mut cursor = Cursor::new(self.heap.find_entry(heap_id)?);
                while let Ok(row) = HeapTreeIntermediateEntry::<K>::read(&mut cursor) {
                    next_level.push(row.next_level());
                }
            }

            level -= 1;
        }

        let mut results = Vec::new();
        for heap_id in mem::take(&mut next_level).into_iter() {
            let mut cursor = Cursor::new(self.heap.find_entry(heap_id)?);
            while let Ok(row) = HeapTreeLeafEntry::<K, V>::read(&mut cursor) {
                results.push(row);
            }
        }

        Ok(results)
    }
}

pub struct UnicodeHeapTree<K, V>
where
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    inner: HeapTreeInner<UnicodePstFile, K, V>,
}

impl<K, V> HeapTree for UnicodeHeapTree<K, V>
where
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    type Key = K;
    type Value = V;

    fn heap(&self) -> &dyn HeapNode {
        &self.inner.heap
    }

    fn user_root(&self) -> HeapId {
        self.inner.user_root
    }

    fn header(&self) -> io::Result<HeapTreeHeader> {
        self.inner.header()
    }

    fn entries(&self) -> io::Result<Vec<HeapTreeLeafEntry<K, V>>> {
        self.inner.entries()
    }
}

impl<K, V> HeapTreeReadWrite<UnicodePstFile> for UnicodeHeapTree<K, V>
where
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    fn new(heap: UnicodeHeapNode, user_root: HeapId) -> Self {
        let inner = HeapTreeInner::new(heap, user_root);
        Self { inner }
    }
}

impl<K, V> From<UnicodeHeapTree<K, V>> for UnicodeHeapNode
where
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    fn from(value: UnicodeHeapTree<K, V>) -> Self {
        value.inner.heap
    }
}

pub struct AnsiHeapTree<K, V>
where
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    inner: HeapTreeInner<AnsiPstFile, K, V>,
}

impl<K, V> HeapTree for AnsiHeapTree<K, V>
where
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    type Key = K;
    type Value = V;

    fn heap(&self) -> &dyn HeapNode {
        &self.inner.heap
    }

    fn user_root(&self) -> HeapId {
        self.inner.user_root
    }

    fn header(&self) -> io::Result<HeapTreeHeader> {
        self.inner.header()
    }

    fn entries(&self) -> io::Result<Vec<HeapTreeLeafEntry<K, V>>> {
        self.inner.entries()
    }
}

impl<K, V> HeapTreeReadWrite<AnsiPstFile> for AnsiHeapTree<K, V>
where
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    fn new(heap: AnsiHeapNode, user_root: HeapId) -> Self {
        let inner = HeapTreeInner::new(heap, user_root);
        Self { inner }
    }
}

impl<K, V> From<AnsiHeapTree<K, V>> for AnsiHeapNode
where
    K: HeapTreeEntryKey + HeapNodePageReadWrite,
    V: HeapTreeEntryValue + HeapNodePageReadWrite,
{
    fn from(value: AnsiHeapTree<K, V>) -> Self {
        value.inner.heap
    }
}
