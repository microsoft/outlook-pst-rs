#![allow(dead_code, unused_imports)]

use std::{
    cmp::Ordering,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
};

use super::{heap::*, property::*, table::*, tree::*, *};

pub trait HeapIdReadWrite: Copy + Sized {
    fn new(index: u16, block_index: u16) -> LtpResult<Self>;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait HeapNodeReadWrite: Sized {
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}
