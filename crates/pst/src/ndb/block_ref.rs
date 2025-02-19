//! [BREF](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/844a5ebf-488a-45fd-8fce-92a84d8e24a3)

use std::io::{self, Read, Write};

use super::*;

pub trait BlockRef: Sized + Copy {
    type Block: BlockId;
    type Index: ByteIndex;

    fn new(block: Self::Block, index: Self::Index) -> Self;
    fn block(&self) -> &Self::Block;
    fn index(&self) -> &Self::Index;

    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let block = Self::Block::read(f)?;
        let index = Self::Index::read(f)?;
        Ok(Self::new(block, index))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        self.block().write(f)?;
        self.index().write(f)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct UnicodeBlockRef {
    block: UnicodeBlockId,
    index: UnicodeByteIndex,
}

impl BlockRef for UnicodeBlockRef {
    type Block = UnicodeBlockId;
    type Index = UnicodeByteIndex;

    fn new(block: UnicodeBlockId, index: UnicodeByteIndex) -> Self {
        Self { block, index }
    }

    fn block(&self) -> &UnicodeBlockId {
        &self.block
    }

    fn index(&self) -> &UnicodeByteIndex {
        &self.index
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct AnsiBlockRef {
    block: AnsiBlockId,
    index: AnsiByteIndex,
}

impl BlockRef for AnsiBlockRef {
    type Block = AnsiBlockId;
    type Index = AnsiByteIndex;

    fn new(block: AnsiBlockId, index: AnsiByteIndex) -> Self {
        Self { block, index }
    }

    fn block(&self) -> &AnsiBlockId {
        &self.block
    }

    fn index(&self) -> &AnsiByteIndex {
        &self.index
    }
}
