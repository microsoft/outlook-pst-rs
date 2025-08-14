//! [ROOT](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/32ce8c94-4757-46c8-a169-3fd21abee584)

use std::fmt::Debug;

use super::{block_ref::*, byte_index::*, read_write::*, *};
use crate::{AnsiPstFile, PstFile, UnicodePstFile};

/// `fAMapValid`
///
/// ### See also
/// [Root]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum AmapStatus {
    /// `INVALID_AMAP`: One or more AMaps in the PST are INVALID
    #[default]
    Invalid = 0x00,
    /// `VALID_AMAP1`: Deprecated. Implementations SHOULD NOT use this value. The AMaps are VALID.
    Valid1 = 0x01,
    /// `VALID_AMAP2`: The AMaps are VALID.
    Valid2 = 0x02,
}

impl TryFrom<u8> for AmapStatus {
    type Error = NdbError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(AmapStatus::Invalid),
            0x01 => Ok(AmapStatus::Valid1),
            0x02 => Ok(AmapStatus::Valid2),
            _ => Err(NdbError::InvalidAmapStatus(value)),
        }
    }
}

impl From<AmapStatus> for bool {
    fn from(status: AmapStatus) -> bool {
        status != AmapStatus::Invalid
    }
}

pub trait Root<Pst>
where
    Pst: PstFile,
{
    fn file_eof_index(&self) -> &<Pst as PstFile>::ByteIndex;
    fn amap_last_index(&self) -> &<Pst as PstFile>::ByteIndex;
    fn amap_free_size(&self) -> &<Pst as PstFile>::ByteIndex;
    fn pmap_free_size(&self) -> &<Pst as PstFile>::ByteIndex;
    fn node_btree(&self) -> &<Pst as PstFile>::PageRef;
    fn block_btree(&self) -> &<Pst as PstFile>::PageRef;
    fn amap_is_valid(&self) -> AmapStatus;
}

#[derive(Clone, Debug)]
pub struct UnicodeRoot {
    reserved1: u32,
    file_eof_index: UnicodeByteIndex,
    amap_last_index: UnicodeByteIndex,
    amap_free_size: UnicodeByteIndex,
    pmap_free_size: UnicodeByteIndex,
    node_btree: UnicodePageRef,
    block_btree: UnicodePageRef,
    amap_is_valid: AmapStatus,
    reserved2: u8,
    reserved3: u16,
}

impl UnicodeRoot {
    pub fn new(
        file_eof_index: UnicodeByteIndex,
        amap_last_index: UnicodeByteIndex,
        amap_free_size: UnicodeByteIndex,
        pmap_free_size: UnicodeByteIndex,
        node_btree: UnicodePageRef,
        block_btree: UnicodePageRef,
        amap_is_valid: AmapStatus,
    ) -> Self {
        Self {
            reserved1: Default::default(),
            file_eof_index,
            amap_last_index,
            amap_free_size,
            pmap_free_size,
            node_btree,
            block_btree,
            amap_is_valid,
            reserved2: Default::default(),
            reserved3: Default::default(),
        }
    }
}

impl Root<UnicodePstFile> for UnicodeRoot {
    fn file_eof_index(&self) -> &UnicodeByteIndex {
        &self.file_eof_index
    }

    fn amap_last_index(&self) -> &UnicodeByteIndex {
        &self.amap_last_index
    }

    fn amap_free_size(&self) -> &UnicodeByteIndex {
        &self.amap_free_size
    }

    fn pmap_free_size(&self) -> &UnicodeByteIndex {
        &self.pmap_free_size
    }

    fn node_btree(&self) -> &UnicodePageRef {
        &self.node_btree
    }

    fn block_btree(&self) -> &UnicodePageRef {
        &self.block_btree
    }

    fn amap_is_valid(&self) -> AmapStatus {
        self.amap_is_valid
    }
}

impl RootReadWrite<UnicodePstFile> for UnicodeRoot {
    fn new(
        file_eof_index: UnicodeByteIndex,
        amap_last_index: UnicodeByteIndex,
        amap_free_size: UnicodeByteIndex,
        pmap_free_size: UnicodeByteIndex,
        node_btree: UnicodePageRef,
        block_btree: UnicodePageRef,
        amap_is_valid: AmapStatus,
    ) -> Self {
        Self::new(
            file_eof_index,
            amap_last_index,
            amap_free_size,
            pmap_free_size,
            node_btree,
            block_btree,
            amap_is_valid,
        )
    }

    fn load_reserved(&mut self, reserved1: u32, reserved2: u8, reserved3: u16) {
        self.reserved1 = reserved1;
        self.reserved2 = reserved2;
        self.reserved3 = reserved3;
    }

    fn reserved1(&self) -> u32 {
        self.reserved1
    }

    fn reserved2(&self) -> u8 {
        self.reserved2
    }

    fn reserved3(&self) -> u16 {
        self.reserved3
    }

    fn set_amap_status(&mut self, status: AmapStatus) {
        self.amap_is_valid = status;
    }

    fn reset_free_size(&mut self, free_bytes: UnicodeByteIndex) -> NdbResult<()> {
        self.amap_free_size = free_bytes;
        self.pmap_free_size = 0.into();
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AnsiRoot {
    file_eof_index: AnsiByteIndex,
    amap_last_index: AnsiByteIndex,
    amap_free_size: AnsiByteIndex,
    pmap_free_size: AnsiByteIndex,
    node_btree: AnsiPageRef,
    block_btree: AnsiPageRef,
    amap_is_valid: AmapStatus,
    reserved1: u32,
    reserved2: u8,
    reserved3: u16,
}

impl AnsiRoot {
    pub fn new(
        file_eof_index: AnsiByteIndex,
        amap_last_index: AnsiByteIndex,
        amap_free_size: AnsiByteIndex,
        pmap_free_size: AnsiByteIndex,
        node_btree: AnsiPageRef,
        block_btree: AnsiPageRef,
        amap_is_valid: AmapStatus,
    ) -> Self {
        Self {
            reserved1: Default::default(),
            file_eof_index,
            amap_last_index,
            amap_free_size,
            pmap_free_size,
            node_btree,
            block_btree,
            amap_is_valid,
            reserved2: Default::default(),
            reserved3: Default::default(),
        }
    }
}

impl Root<AnsiPstFile> for AnsiRoot {
    fn file_eof_index(&self) -> &AnsiByteIndex {
        &self.file_eof_index
    }

    fn amap_last_index(&self) -> &AnsiByteIndex {
        &self.amap_last_index
    }

    fn amap_free_size(&self) -> &AnsiByteIndex {
        &self.amap_free_size
    }

    fn pmap_free_size(&self) -> &AnsiByteIndex {
        &self.pmap_free_size
    }

    fn node_btree(&self) -> &AnsiPageRef {
        &self.node_btree
    }

    fn block_btree(&self) -> &AnsiPageRef {
        &self.block_btree
    }

    fn amap_is_valid(&self) -> AmapStatus {
        self.amap_is_valid
    }
}

impl RootReadWrite<AnsiPstFile> for AnsiRoot {
    fn new(
        file_eof_index: AnsiByteIndex,
        amap_last_index: AnsiByteIndex,
        amap_free_size: AnsiByteIndex,
        pmap_free_size: AnsiByteIndex,
        node_btree: AnsiPageRef,
        block_btree: AnsiPageRef,
        amap_is_valid: AmapStatus,
    ) -> Self {
        Self::new(
            file_eof_index,
            amap_last_index,
            amap_free_size,
            pmap_free_size,
            node_btree,
            block_btree,
            amap_is_valid,
        )
    }

    fn load_reserved(&mut self, reserved1: u32, reserved2: u8, reserved3: u16) {
        self.reserved1 = reserved1;
        self.reserved2 = reserved2;
        self.reserved3 = reserved3;
    }

    fn reserved1(&self) -> u32 {
        self.reserved1
    }

    fn reserved2(&self) -> u8 {
        self.reserved2
    }

    fn reserved3(&self) -> u16 {
        self.reserved3
    }

    fn set_amap_status(&mut self, status: AmapStatus) {
        self.amap_is_valid = status;
    }

    fn reset_free_size(&mut self, free_bytes: AnsiByteIndex) -> NdbResult<()> {
        self.amap_free_size = free_bytes;
        self.pmap_free_size = 0.into();
        Ok(())
    }
}
