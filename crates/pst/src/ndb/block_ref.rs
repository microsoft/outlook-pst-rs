//! [BREF](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/844a5ebf-488a-45fd-8fce-92a84d8e24a3)

use super::{block_id::*, byte_index::*, read_write::*};

pub trait BlockRef
where
    u64: From<<Self::Block as BlockId>::Index> + From<<Self::Index as ByteIndex>::Index>,
{
    type Block: BlockId;
    type Index: ByteIndex;

    fn block(&self) -> Self::Block;
    fn index(&self) -> Self::Index;
}

#[derive(Clone, Copy, Default, Debug)]
pub struct UnicodeBlockRef {
    block: UnicodeBlockId,
    index: UnicodeByteIndex,
}

impl UnicodeBlockRef {
    pub fn new(block: UnicodeBlockId, index: UnicodeByteIndex) -> Self {
        Self { block, index }
    }
}

impl BlockRef for UnicodeBlockRef {
    type Block = UnicodeBlockId;
    type Index = UnicodeByteIndex;

    fn block(&self) -> UnicodeBlockId {
        self.block
    }

    fn index(&self) -> UnicodeByteIndex {
        self.index
    }
}

impl BlockRefReadWrite for UnicodeBlockRef {
    fn new(block: UnicodeBlockId, index: UnicodeByteIndex) -> Self {
        Self::new(block, index)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct AnsiBlockRef {
    block: AnsiBlockId,
    index: AnsiByteIndex,
}

impl AnsiBlockRef {
    pub fn new(block: AnsiBlockId, index: AnsiByteIndex) -> Self {
        Self { block, index }
    }
}

impl BlockRef for AnsiBlockRef {
    type Block = AnsiBlockId;
    type Index = AnsiByteIndex;

    fn block(&self) -> AnsiBlockId {
        self.block
    }

    fn index(&self) -> AnsiByteIndex {
        self.index
    }
}

impl BlockRefReadWrite for AnsiBlockRef {
    fn new(block: AnsiBlockId, index: AnsiByteIndex) -> Self {
        Self::new(block, index)
    }
}
