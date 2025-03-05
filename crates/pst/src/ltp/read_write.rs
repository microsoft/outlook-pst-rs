#![allow(dead_code)]

use std::io::{self, Read, Write};

use super::{prop_type::*, table_context::*, *};

pub trait HeapIdReadWrite: Copy + Sized {
    fn new(index: u16, block_index: u16) -> LtpResult<Self>;
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait HeapNodeReadWrite: Sized {
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait PropertyTreeRecordReadWrite: Sized {
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait PropertyValueReadWrite: Sized {
    fn read(f: &mut dyn Read, prop_type: PropertyType) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait TableContextReadWrite: Sized {
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait TableRowReadWrite: Sized {
    fn read(f: &mut dyn Read, context: &TableContextInfo) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}
