//! ## [Messaging Layer](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/3f1bc553-d15d-4dcf-9b80-fbf1dd6c7e79)

use std::io;
use thiserror::Error;

pub mod store;

pub(crate) mod read_write;

#[derive(Error, Debug)]
pub enum MessagingError {
    #[error("Node Database error: {0}")]
    NodeDatabaseError(#[from] crate::ndb::NdbError),
    #[error("Node Database error: {0}")]
    ListsTablesPropertiesError(#[from] crate::ltp::LtpError),
    #[error("Failed to lock PST file")]
    FailedToLockFile,
    #[error("Invalid EntryID rgbFlags: 0x{0:08X}")]
    InvalidEntryIdFlags(u32),
    #[error("Missing PidTagRecordKey on store")]
    StoreRecordKeyNotFound,
    #[error("Invalid PidTagRecordKey on store: {0:?}")]
    InvalidStoreRecordKey(crate::ltp::prop_type::PropertyType),
    #[error("Invalid PidTagRecordKey size on store: 0x{0:X}")]
    InvalidStoreRecordKeySize(usize),
    #[error("Missing PidTagDisplayName on store")]
    StoreDisplayNameNotFound,
    #[error("Invalid PidTagDisplayName on store: {0:?}")]
    InvalidStoreDisplayName(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagIpmSubTreeEntryId on store")]
    StoreIpmSubTreeEntryIdNotFound,
    #[error("Invalid PidTagIpmSubTreeEntryId on store: {0:?}")]
    StoreInvalidIpmSubTreeEntryId(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagIpmWastebasketEntryId on store")]
    StoreIpmWastebasketEntryIdNotFound,
    #[error("Invalid PidTagIpmWastebasketEntryId on store: {0:?}")]
    StoreInvalidIpmWastebasketEntryId(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagFinderEntryId on store")]
    StoreFinderEntryIdNotFound,
    #[error("Invalid PidTagFinderEntryId on store: {0:?}")]
    StoreInvalidFinderEntryId(crate::ltp::prop_type::PropertyType),
}

impl From<MessagingError> for io::Error {
    fn from(err: MessagingError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

pub type MessagingResult<T> = Result<T, MessagingError>;
