//! ## [Messaging Layer](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/3f1bc553-d15d-4dcf-9b80-fbf1dd6c7e79)

use std::io;
use thiserror::Error;

pub mod attachment;
pub mod folder;
pub mod message;
pub mod named_prop;
pub mod search;
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
    #[error("Failed to read root hierarchy table: {0}")]
    StoreRootHierarchyTableFailed(String),
    #[error("Failed to open a folder: {0}")]
    StoreOpenFolder(String),
    #[error("Failed to read named property map: {0}")]
    StoreNamedPropertyMap(String),
    #[error("Failed to read search update queue: {0}")]
    StoreSearchUpdateQueue(String),
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
    #[error("Multiple NID_TYPE_RECIPIENT_TABLE sub-nodes on message")]
    MultipleMessageRecipientTables,
    #[error("Multiple NID_TYPE_ATTACHMENT_TABLE sub-nodes on message")]
    MultipleMessageAttachmentTables,
    #[error("Missing PidTagAttachSize on message")]
    AttachmentSizeNotFound,
    #[error("Invalid PidTagAttachSize on message: {0:?}")]
    InvalidAttachmentSize(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagAttachMethod on message")]
    AttachmentMethodNotFound,
    #[error("Invalid PidTagAttachMethod on message: {0:?}")]
    InvalidAttachmentMethod(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagRenderingPosition on message")]
    AttachmentRenderingPositionNotFound,
    #[error("Invalid PidTagRenderingPosition on message: {0:?}")]
    InvalidAttachmentRenderingPosition(crate::ltp::prop_type::PropertyType),
    #[error("Invalid attachment Sub-Node NID_TYPE: {0:?}")]
    InvalidAttachmentNodeIdType(crate::ndb::node_id::NodeIdType),
    #[error("Unrecognized PidTagAttachMethod on attachment: 0x{0:08X}")]
    UnknownAttachmentMethod(i32),
    #[error("Missing attachment Sub-Node on message: {0:?}")]
    AttachmentSubNodeNotFound(crate::ndb::node_id::NodeId),
    #[error("Missing PidTagAttachDataObject on afEmbeddedMessage attachment")]
    AttachmentMessageObjectDataNotFound,
    #[error("Invalid PidTagAttachDataObject on afEmbeddedMessage attachment: {0:?}")]
    InvalidMessageObjectData(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagAttachDataBinary on afByValue attachment")]
    AttachmentFileBinaryDataNotFound,
    #[error("Invalid PidTagAttachDataBinary on afByValue attachment: {0:?}")]
    InvalidAttachmentFileBinaryData(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagAttachDataObject on afStorage attachment")]
    AttachmentStorageObjectDataNotFound,
    #[error("Reading afStorage attachment failed: {0}")]
    AttachmentStorageRead(String),
    #[error("Invalid PidTagAttachDataObject on afStorage attachment: {0:?}")]
    InvalidStorageObjectData(crate::ltp::prop_type::PropertyType),
    #[error("NAMEID wGuid is out of bounds: 0x{0:04X}")]
    NamedPropertyMapGuidIndexOutOfBounds(u16),
    #[error("NAMEID wPropIdx is out of bounds: 0x{0:04X}")]
    NamedPropertyMapPropertyIndexOutOfBounds(u16),
    #[error("PidTagNameidBucketCount on Named Property Lookup Map is out of bounds: 0x{0:08X}")]
    NamedPropertyMapBucketCountOutOfBounds(i32),
    #[error("Hash bucket offset on Named Property Lookup Map is out of bounds: 0x{0:04X}")]
    NamedPropertyMapBucketOffsetOutOfBounds(u32),
    #[error("Invalid PidTagNameidStreamString string")]
    NamedPropertyMapStringEntryOutOfBounds,
    #[error("Missing PidTagNameidBucketCount on Named Property Lookup Map")]
    NamedPropertyMapBucketCountNotFound,
    #[error("Invalid PidTagNameidBucketCount on Named Property Lookup Map: {0:?}")]
    InvalidNamedPropertyMapBucketCount(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagNameidStreamGuid on Named Property Lookup Map")]
    NamedPropertyMapStreamGuidNotFound,
    #[error("Invalid PidTagNameidStreamGuid on Named Property Lookup Map: {0:?}")]
    InvalidNamedPropertyMapStreamGuid(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagNameidStreamEntry on Named Property Lookup Map")]
    NamedPropertyMapStreamEntryNotFound,
    #[error("Invalid PidTagNameidStreamEntry on Named Property Lookup Map: {0:?}")]
    InvalidNamedPropertyMapStreamEntry(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagNameidStreamString on Named Property Lookup Map")]
    NamedPropertyMapStreamStringNotFound,
    #[error("Invalid PidTagNameidStreamString on Named Property Lookup Map: {0:?}")]
    InvalidNamedPropertyMapStreamString(crate::ltp::prop_type::PropertyType),
    #[error("Missing PidTagNameidBucketBase + hash on Named Property Lookup Map")]
    NamedPropertyMapBucketNotFound(u16),
    #[error("Invalid PidTagNameidBucketBase + hash on Named Property Lookup Map: {0:?}")]
    InvalidNamedPropertyMapBucket(crate::ltp::prop_type::PropertyType),
    #[error("Invalid SUD wSUDType: 0x{0:04X}")]
    InvalidSearchUpdateType(u16),
    #[error("Invalid SUD queue offset: 0x{0:08X}")]
    InvalidSearchUpdateQueueOffset(u32),
    #[error("Invalid SUD queue size: {0}")]
    InvalidSearchUpdateQueueSize(usize),
}

impl From<MessagingError> for io::Error {
    fn from(err: MessagingError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

pub type MessagingResult<T> = Result<T, MessagingError>;
