//! ## [Lists, Tables, and Properties (LTP) Layer](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/4c24c7d2-5c5a-4b99-88b2-f4b84cc293ae)

use std::io;
use thiserror::Error;

pub mod heap;
pub mod property;
pub mod table;
pub mod tree;

pub(crate) mod read_write;

#[derive(Error, Debug)]
pub enum LtpError {
    #[error("Node Database error: {0}")]
    NodeDatabaseError(#[from] crate::ndb::NdbError),
    #[error("Invalid HID hidIndex: 0x{0:04X}")]
    InvalidHeapIndex(u16),
    #[error("Invalid HID hidType: {0:?}")]
    InvalidHeapNodeType(crate::ndb::node_id::NodeIdType),
    #[error("Invalid HNHDR bClientSig: 0x{0:02X}")]
    InvalidHeapClientSignature(u8),
    #[error("Invalid HNHDR rgbFillLevel: 0x{0:02X}")]
    InvalidHeapFillLevel(u8),
    #[error("HNPAGEMAP is out of space")]
    HeapPageOutOfSpace,
    #[error("Empty HNPAGEMAP rgibAlloc")]
    EmptyHeapPageAlloc,
    #[error("Invalid HNPAGEMAP rgibAlloc entry: 0x{0:04X}")]
    InvalidHeapPageAllocOffset(u16),
    #[error("Invalid HNPAGEMAP cAlloc: 0x{0:04X}")]
    InvalidHeapPageAllocCount(u16),
    #[error("Invalid HNPAGEMAP cFree: 0x{0:04X}")]
    InvalidHeapPageFreeCount(u16),
}

impl From<LtpError> for io::Error {
    fn from(err: LtpError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

pub type LtpResult<T> = Result<T, LtpError>;
