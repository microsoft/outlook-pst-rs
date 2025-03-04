//! ## [Messaging Layer](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/3f1bc553-d15d-4dcf-9b80-fbf1dd6c7e79)

use std::io;
use thiserror::Error;

pub mod folder;
pub mod message;
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
    InvalidStoreIpmSubTreeEntryId(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagIpmWastebasketEntryId on store")]
    StoreIpmWastebasketEntryIdNotFound,
    #[error("Invalid PidTagIpmWastebasketEntryId on store: {0:?}")]
    InvalidStoreIpmWastebasketEntryId(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagFinderEntryId on store")]
    StoreFinderEntryIdNotFound,
    #[error("Invalid PidTagFinderEntryId on store: {0:?}")]
    InvalidStoreFinderEntryId(crate::ltp::prop_type::PropertyType),
    #[error("EntryID in wrong store")]
    EntryIdWrongStore,
    #[error("Missing PidTagDisplayName on folder")]
    FolderDisplayNameNotFound,
    #[error("Invalid PidTagDisplayName on folder: {0:?}")]
    InvalidFolderDisplayName(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagContentCount on folder")]
    FolderContentCountNotFound,
    #[error("Invalid PidTagContentCount on folder: {0:?}")]
    InvalidFolderContentCount(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagContentUnreadCount on folder")]
    FolderUnreadCountNotFound,
    #[error("Invalid PidTagContentUnreadCount on folder: {0:?}")]
    InvalidFolderUnreadCount(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagSubfolders on folder")]
    FolderHasSubfoldersNotFound,
    #[error("Invalid PidTagSubfolders on folder: {0:?}")]
    InvalidFolderHasSubfolders(crate::ltp::prop_type::PropertyType),
    #[error("Invalid folder EntryID NID_TYPE: {0:?}")]
    InvalidFolderEntryIdType(crate::ndb::node_id::NodeIdType),
    #[error("Missing PidTagMessageClass on message")]
    MessageClassNotFound,
    #[error("Invalid PidTagMessageClass on message: {0:?}")]
    InvalidMessageClass(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagMessageFlags on message")]
    MessageFlagsNotFound,
    #[error("Invalid PidTagMessageFlags on message: {0:?}")]
    InvalidMessageFlags(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagMessageSize on message")]
    MessageSizeNotFound,
    #[error("Invalid PidTagMessageSize on message: {0:?}")]
    InvalidMessageSize(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagMessageStatus on message")]
    MessageStatusNotFound,
    #[error("Invalid PidTagMessageStatus on message: {0:?}")]
    InvalidMessageStatus(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagMessageCreationTime on message")]
    MessageCreationTimeNotFound,
    #[error("Invalid PidTagMessageCreationTime on message: {0:?}")]
    InvalidMessageCreationTime(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagMessageLastModificationTime on message")]
    MessageLastModificationTimeNotFound,
    #[error("Invalid PidTagMessageLastModificationTime on message: {0:?}")]
    InvalidMessageLastModificationTime(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagMessageSearchKey on message")]
    MessageSearchKeyNotFound,
    #[error("Invalid PidTagMessageSearchKey on message: {0:?}")]
    InvalidMessageSearchKey(crate::ltp::prop_type::PropertyType),
    #[error("Invalid message EntryID NID_TYPE: {0:?}")]
    InvalidMessageEntryIdType(crate::ndb::node_id::NodeIdType),
    #[error("Missing Sub-Node Tree on message")]
    MessageSubNodeTreeNotFound,
}

impl From<MessagingError> for io::Error {
    fn from(err: MessagingError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

pub type MessagingResult<T> = Result<T, MessagingError>;
