//! ## [Search](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/3991391e-6cf6-4c97-8b9e-fc25bee7391b)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::{
    io::{self, Cursor, Read, Write},
    marker::PhantomData,
    rc::Rc,
};

use super::{read_write::*, *};
use crate::{
    messaging::store::{AnsiStore, UnicodeStore},
    ndb::{
        block::{DataTree, IntermediateTreeBlock},
        block_id::BlockId,
        header::Header,
        node_id::{NodeId, NID_SEARCH_MANAGEMENT_QUEUE},
        page::{BTreePage, NodeBTreeEntry, RootBTree},
        read_write::*,
        root::Root,
    },
    AnsiPstFile, PstFile, UnicodePstFile,
};

/// `wFlags`
///
/// ### See also
/// [SearchUpdate]
#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SearchUpdateFlags {
    #[default]
    None = 0x0000,
    /// `SUDF_PRIORITY_LOW`: Change search Folder object priority to foreground.
    /// Applies To: `SUDT_SRCH_MOD`
    PriorityLow = 0x0001,
    /// `SUDF_PRIORITY_HIGH`: Change search Folder object priority to background.
    /// Applies To: `SUDT_SRCH_MOD`
    PriorityHigh = 0x0002,
    /// `SUDF_SEARCH_RESTART`: Request full rebuild of search Folder object contents.
    /// Applies To: `SUDT_SRCH_MOD`
    SearchRestart = 0x0004,
    /// `SUDF_NAME_CHANGED`: Display Name of Folder object changed.
    /// Applies To: `SUDT_FLD_MOD`
    NameChanged = 0x0008,
    /// `SUDF_MOVE_OUT_TO_IN`: Move from non-SDO domain to SDO domain.
    /// Applies To: `SUDT_FLD`/`MSG_MOV`
    MoveOutToIn = 0x0010,
    /// `SUDF_MOVE_IN_TO_IN`: Move from SDO domain to SDO domain.
    /// Applies To: `SUDT_FLD`/`MSG_MOV`
    MoveInToIn = 0x0020,
    /// `SUDF_MOVE_IN_TO_OUT`: Move from SDO domain to non-SDO domain.
    /// Applies To: `SUDT_MSG_MOV`
    MoveInToOut = 0x0040,
    /// `SUDF_MOVE_OUT_TO_OUT`: Move between non-SDO domains.
    /// Applies To: `SUDT_MSG_MOV`
    MoveOutToOut = 0x0080,
    /// `SUDF_SPAM_CHECK_SERVER`: Make sure spam Message object deleted on server.
    /// Applies To: `SUDT_MSG_SPAM`
    SpamCheckServer = 0x0100,
    /// `SUDF_SET_DEL_NAME`: Delegate Root Name of Folder object changed.
    /// Applies To: `SUDT_FLD_MOD`
    SetDelegateRootName = 0x0200,
    /// `SUDF_SRCH_DONE`: Search is finished for associated object.
    /// Applies To: `SUDT_SRCH_MOD`
    SearchDone = 0x0400,
    /// `SUDF_DOMAIN_CHECKED`: Object is validated against the SDO.
    /// Applies To: `SUDT_FLD`/`MSG_*`
    DomainChecked = 0x8000,
}

/// `wSUDType`
///
/// ### See also
/// [SearchUpdate]
#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SearchUpdateType {
    /// `SUDT_NULL`: Invalid SUD Type.
    #[default]
    Null = 0x0000,
    /// `SUDT_MSG_ADD`: Message added.
    MessageAdded = 0x0001,
    /// `SUDT_MSG_MOD`: Message modified.
    MessageModified = 0x0002,
    /// `SUDT_MSG_DEL`: Message deleted.
    MessageDeleted = 0x0003,
    /// `SUDT_MSG_MOV`: Message moved.
    MessageMoved = 0x0004,
    /// `SUDT_FLD_ADD`: Folder object added.
    FolderAdded = 0x0005,
    /// `SUDT_FLD_MOD`: Folder object modified.
    FolderModified = 0x0006,
    /// `SUDT_FLD_DEL`: Folder object deleted.
    FolderDeleted = 0x0007,
    /// `SUDT_FLD_MOV`: Folder object moved.
    FolderMoved = 0x0008,
    /// `SUDT_SRCH_ADD`: Search Folder object added.
    SearchFolderAdded = 0x0009,
    /// `SUDT_SRCH_MOD`: Search Folder object modified.
    SearchFolderModified = 0x000A,
    /// `SUDT_SRCH_DEL`: Search Folder object deleted.
    SearchFolderDeleted = 0x000B,
    /// `SUDT_MSG_ROW_MOD`: Message modified, contents table affected.
    MessageRowModified = 0x000C,
    /// `SUDT_MSG_SPAM`: Message identified as spam.
    MessageSpam = 0x000D,
    /// `SUDT_IDX_MSG_DEL`: Content-indexed Message object deleted.
    IndexedMessageDeleted = 0x000E,
    /// `SUDT_MSG_IDX`: Message has been indexed.
    MessageIndexed = 0x000F,
}

impl TryFrom<u16> for SearchUpdateType {
    type Error = MessagingError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0000 => Ok(Self::Null),
            0x0001 => Ok(Self::MessageAdded),
            0x0002 => Ok(Self::MessageModified),
            0x0003 => Ok(Self::MessageDeleted),
            0x0004 => Ok(Self::MessageMoved),
            0x0005 => Ok(Self::FolderAdded),
            0x0006 => Ok(Self::FolderModified),
            0x0007 => Ok(Self::FolderDeleted),
            0x0008 => Ok(Self::FolderMoved),
            0x0009 => Ok(Self::SearchFolderAdded),
            0x000A => Ok(Self::SearchFolderModified),
            0x000B => Ok(Self::SearchFolderDeleted),
            0x000C => Ok(Self::MessageRowModified),
            0x000D => Ok(Self::MessageSpam),
            0x000E => Ok(Self::IndexedMessageDeleted),
            0x000F => Ok(Self::MessageIndexed),
            invalid => Err(MessagingError::InvalidSearchUpdateType(invalid)),
        }
    }
}

/// [SUDData Structures](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/4d919e3b-33b3-46fa-b2ff-17fbc324b12b)
#[derive(Clone, Copy, Debug)]
pub enum SearchUpdateData {
    /// [SUD_MSG_ADD](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/c0a889d8-6a34-431f-8305-91f836620cdb)
    MessageAdded { parent: NodeId, message: NodeId },
    /// [SUD_MSG_MOD](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/c0a889d8-6a34-431f-8305-91f836620cdb)
    MessageModified { parent: NodeId, message: NodeId },
    /// [SUD_MSG_DEL](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/c0a889d8-6a34-431f-8305-91f836620cdb)
    MessageDeleted { parent: NodeId, message: NodeId },
    /// [SUD_MSG_MOV](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/ed48b4c0-a034-4818-9e28-8488c8c30681)
    MessageMoved {
        new_parent: NodeId,
        message: NodeId,
        old_parent: NodeId,
    },
    /// [SUD_FLD_ADD](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/f8d4750e-9721-479d-acf5-43c902919e0d)
    FolderAdded {
        parent: NodeId,
        folder: NodeId,
        reserved1: u32,
        reserved2: u32,
    },
    /// [SUD_FLD_MOD](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/249e61f9-d192-42e3-b5bc-9eecc7f2d5e3)
    FolderModified { folder: NodeId, reserved: u32 },
    /// [SUDT_FLD_DEL](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/249e61f9-d192-42e3-b5bc-9eecc7f2d5e3)
    FolderDeleted { folder: NodeId, reserved: u32 },
    /// [SUD_FLD_MOV](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/f8d4750e-9721-479d-acf5-43c902919e0d)
    FolderMoved {
        parent: NodeId,
        folder: NodeId,
        reserved1: u32,
        reserved2: u32,
    },
    /// [SUDT_SRCH_ADD](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/f795bd47-d658-47e1-aa35-f921fa0da8f9)
    SearchFolderAdded { search_folder: NodeId },
    /// [SUDT_SRCH_MOD](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/f540791d-b5b5-41fe-8b3c-43cdaf1ef12c)
    SearchFolderModified {
        search_folder: NodeId,
        reserved: u32,
    },
    /// [SUDT_SRCH_DEL](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/f795bd47-d658-47e1-aa35-f921fa0da8f9)
    SearchFolderDeleted { search_folder: NodeId },
    /// [SUD_MSG_MOD](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/c0a889d8-6a34-431f-8305-91f836620cdb)
    MessageRowModified { parent: NodeId, message: NodeId },
    /// [SUD_MSG_SPAM](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/f3776950-d0d4-43d3-9d13-b4c4ae8fe16f)
    MessageSpam { parent: NodeId, message: NodeId },
    /// [SUDT_IDX_MSG_DEL](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/f3776950-d0d4-43d3-9d13-b4c4ae8fe16f)
    IndexedMessageDeleted { parent: NodeId, message: NodeId },
    /// [SUDT_MSG_IDX](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/cb466e74-75e2-4e22-a474-197592fcb93f)
    MessageIndexed { message: NodeId },
}

impl From<&SearchUpdateData> for SearchUpdateType {
    fn from(value: &SearchUpdateData) -> Self {
        match value {
            SearchUpdateData::MessageAdded { .. } => Self::MessageAdded,
            SearchUpdateData::MessageModified { .. } => Self::MessageModified,
            SearchUpdateData::MessageDeleted { .. } => Self::MessageDeleted,
            SearchUpdateData::MessageMoved { .. } => Self::MessageMoved,
            SearchUpdateData::FolderAdded { .. } => Self::FolderAdded,
            SearchUpdateData::FolderModified { .. } => Self::FolderModified,
            SearchUpdateData::FolderDeleted { .. } => Self::FolderDeleted,
            SearchUpdateData::FolderMoved { .. } => Self::FolderMoved,
            SearchUpdateData::SearchFolderAdded { .. } => Self::SearchFolderAdded,
            SearchUpdateData::SearchFolderModified { .. } => Self::SearchFolderModified,
            SearchUpdateData::SearchFolderDeleted { .. } => Self::SearchFolderDeleted,
            SearchUpdateData::MessageRowModified { .. } => Self::MessageRowModified,
            SearchUpdateData::MessageSpam { .. } => Self::MessageSpam,
            SearchUpdateData::IndexedMessageDeleted { .. } => Self::IndexedMessageDeleted,
            SearchUpdateData::MessageIndexed { .. } => Self::MessageIndexed,
        }
    }
}

/// [SUD Structure](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/ea392b3c-48ca-442b-98c6-f38f5d66f93b)
#[derive(Clone, Copy, Debug)]
pub struct SearchUpdate {
    flags: u16,
    data: Option<SearchUpdateData>,
}

impl SearchUpdate {
    pub fn flags(&self) -> u16 {
        self.flags
    }

    pub fn data(&self) -> Option<&SearchUpdateData> {
        self.data.as_ref()
    }
}

impl SearchReadWrite for SearchUpdate {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let flags = f.read_u16::<LittleEndian>()?;
        let data_type = SearchUpdateType::try_from(f.read_u16::<LittleEndian>()?)?;

        let mut buffer = [0u8; 16];
        f.read_exact(&mut buffer)?;
        let mut cursor = Cursor::new(buffer);

        let data = match data_type {
            SearchUpdateType::Null => None,
            SearchUpdateType::MessageAdded => {
                let parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let message = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::MessageAdded { parent, message })
            }
            SearchUpdateType::MessageModified => {
                let parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let message = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::MessageModified { parent, message })
            }
            SearchUpdateType::MessageDeleted => {
                let parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let message = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::MessageDeleted { parent, message })
            }
            SearchUpdateType::MessageMoved => {
                let new_parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let message = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let old_parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::MessageMoved {
                    new_parent,
                    message,
                    old_parent,
                })
            }
            SearchUpdateType::FolderAdded => {
                let parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let folder = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let reserved1 = cursor.read_u32::<LittleEndian>()?;
                let reserved2 = cursor.read_u32::<LittleEndian>()?;
                Some(SearchUpdateData::FolderAdded {
                    parent,
                    folder,
                    reserved1,
                    reserved2,
                })
            }
            SearchUpdateType::FolderModified => {
                let folder = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let reserved = cursor.read_u32::<LittleEndian>()?;
                Some(SearchUpdateData::FolderModified { folder, reserved })
            }
            SearchUpdateType::FolderDeleted => {
                let folder = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let reserved = cursor.read_u32::<LittleEndian>()?;
                Some(SearchUpdateData::FolderDeleted { folder, reserved })
            }
            SearchUpdateType::FolderMoved => {
                let parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let folder = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let reserved1 = cursor.read_u32::<LittleEndian>()?;
                let reserved2 = cursor.read_u32::<LittleEndian>()?;
                Some(SearchUpdateData::FolderMoved {
                    parent,
                    folder,
                    reserved1,
                    reserved2,
                })
            }
            SearchUpdateType::SearchFolderAdded => {
                let search_folder = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::SearchFolderAdded { search_folder })
            }
            SearchUpdateType::SearchFolderModified => {
                let search_folder = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let reserved = cursor.read_u32::<LittleEndian>()?;
                Some(SearchUpdateData::SearchFolderModified {
                    search_folder,
                    reserved,
                })
            }
            SearchUpdateType::SearchFolderDeleted => {
                let search_folder = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::SearchFolderDeleted { search_folder })
            }
            SearchUpdateType::MessageRowModified => {
                let parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let message = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::MessageRowModified { parent, message })
            }
            SearchUpdateType::MessageSpam => {
                let parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let message = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::MessageSpam { parent, message })
            }
            SearchUpdateType::IndexedMessageDeleted => {
                let parent = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                let message = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::IndexedMessageDeleted { parent, message })
            }
            SearchUpdateType::MessageIndexed => {
                let message = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                Some(SearchUpdateData::MessageIndexed { message })
            }
        };

        Ok(Self { flags, data })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.flags)?;

        let data_type = self
            .data
            .as_ref()
            .map(SearchUpdateType::from)
            .unwrap_or(SearchUpdateType::Null);
        f.write_u16::<LittleEndian>(data_type as u16)?;

        let mut cursor = Cursor::new([0u8; 16]);
        if let Some(data) = &self.data {
            match *data {
                SearchUpdateData::MessageAdded { parent, message } => {
                    cursor.write_u32::<LittleEndian>(parent.into())?;
                    cursor.write_u32::<LittleEndian>(message.into())?;
                }
                SearchUpdateData::MessageModified { parent, message } => {
                    cursor.write_u32::<LittleEndian>(parent.into())?;
                    cursor.write_u32::<LittleEndian>(message.into())?;
                }
                SearchUpdateData::MessageDeleted { parent, message } => {
                    cursor.write_u32::<LittleEndian>(parent.into())?;
                    cursor.write_u32::<LittleEndian>(message.into())?;
                }
                SearchUpdateData::MessageMoved {
                    new_parent,
                    message,
                    old_parent,
                } => {
                    cursor.write_u32::<LittleEndian>(new_parent.into())?;
                    cursor.write_u32::<LittleEndian>(message.into())?;
                    cursor.write_u32::<LittleEndian>(old_parent.into())?;
                }
                SearchUpdateData::FolderAdded {
                    parent,
                    folder,
                    reserved1,
                    reserved2,
                } => {
                    cursor.write_u32::<LittleEndian>(parent.into())?;
                    cursor.write_u32::<LittleEndian>(folder.into())?;
                    cursor.write_u32::<LittleEndian>(reserved1)?;
                    cursor.write_u32::<LittleEndian>(reserved2)?;
                }
                SearchUpdateData::FolderModified { folder, reserved } => {
                    cursor.write_u32::<LittleEndian>(folder.into())?;
                    cursor.write_u32::<LittleEndian>(reserved)?;
                }
                SearchUpdateData::FolderDeleted { folder, reserved } => {
                    cursor.write_u32::<LittleEndian>(folder.into())?;
                    cursor.write_u32::<LittleEndian>(reserved)?;
                }
                SearchUpdateData::FolderMoved {
                    parent,
                    folder,
                    reserved1,
                    reserved2,
                } => {
                    cursor.write_u32::<LittleEndian>(parent.into())?;
                    cursor.write_u32::<LittleEndian>(folder.into())?;
                    cursor.write_u32::<LittleEndian>(reserved1)?;
                    cursor.write_u32::<LittleEndian>(reserved2)?;
                }
                SearchUpdateData::SearchFolderAdded { search_folder } => {
                    cursor.write_u32::<LittleEndian>(search_folder.into())?;
                }
                SearchUpdateData::SearchFolderModified {
                    search_folder,
                    reserved,
                } => {
                    cursor.write_u32::<LittleEndian>(search_folder.into())?;
                    cursor.write_u32::<LittleEndian>(reserved)?;
                }
                SearchUpdateData::SearchFolderDeleted { search_folder } => {
                    cursor.write_u32::<LittleEndian>(search_folder.into())?;
                }
                SearchUpdateData::MessageRowModified { parent, message } => {
                    cursor.write_u32::<LittleEndian>(parent.into())?;
                    cursor.write_u32::<LittleEndian>(message.into())?;
                }
                SearchUpdateData::MessageSpam { parent, message } => {
                    cursor.write_u32::<LittleEndian>(parent.into())?;
                    cursor.write_u32::<LittleEndian>(message.into())?;
                }
                SearchUpdateData::IndexedMessageDeleted { parent, message } => {
                    cursor.write_u32::<LittleEndian>(parent.into())?;
                    cursor.write_u32::<LittleEndian>(message.into())?;
                }
                SearchUpdateData::MessageIndexed { message } => {
                    cursor.write_u32::<LittleEndian>(message.into())?;
                }
            };
        }

        let data = cursor.into_inner();
        f.write_all(&data)
    }
}

const SEARCH_UPDATE_SIZE: u32 = 20;

pub trait SearchUpdateQueue {
    fn updates(&self) -> &[SearchUpdate];
}

struct SearchUpdateQueueInner<Pst>
where
    Pst: PstFile,
{
    updates: Vec<SearchUpdate>,
    _phantom: PhantomData<Pst>,
}

impl<Pst> SearchUpdateQueueInner<Pst>
where
    Pst: PstFile,
    <Pst as PstFile>::BTreeKey: BTreePageKeyReadWrite,
    u64: From<Pst::BTreeKey>,
    <Pst as PstFile>::NodeBTreeEntry: NodeBTreeEntryReadWrite,
    <Pst as PstFile>::NodeBTree: RootBTreeReadWrite,
    <<Pst as PstFile>::NodeBTree as RootBTree>::IntermediatePage:
        RootBTreeIntermediatePageReadWrite<
            Pst,
            <Pst as PstFile>::NodeBTreeEntry,
            <<Pst as PstFile>::NodeBTree as RootBTree>::LeafPage,
        >,
    <<<Pst as PstFile>::NodeBTree as RootBTree>::IntermediatePage as BTreePage>::Entry:
        BTreePageEntryReadWrite,
    <<Pst as PstFile>::NodeBTree as RootBTree>::LeafPage: RootBTreeLeafPageReadWrite<Pst>,
    <Pst as PstFile>::BlockBTreeEntry: BlockBTreeEntryReadWrite,
    <Pst as PstFile>::BlockBTree: RootBTreeReadWrite,
    <<Pst as PstFile>::BlockBTree as RootBTree>::Entry: BTreeEntryReadWrite,
    <<Pst as PstFile>::BlockBTree as RootBTree>::IntermediatePage:
        RootBTreeIntermediatePageReadWrite<
            Pst,
            <<Pst as PstFile>::BlockBTree as RootBTree>::Entry,
            <<Pst as PstFile>::BlockBTree as RootBTree>::LeafPage,
        >,
    <<Pst as PstFile>::BlockBTree as RootBTree>::LeafPage:
        RootBTreeLeafPageReadWrite<Pst> + BTreePageReadWrite,
    <Pst as PstFile>::BlockTrailer: BlockTrailerReadWrite,
    <Pst as PstFile>::BlockTrailer: BlockTrailerReadWrite,
    <Pst as PstFile>::DataTreeBlock: IntermediateTreeBlockReadWrite,
    <<Pst as PstFile>::DataTreeBlock as IntermediateTreeBlock>::Entry:
        IntermediateTreeEntryReadWrite,
    <Pst as PstFile>::DataBlock: BlockReadWrite + Clone,
    <Pst as PstFile>::Store: StoreReadWrite<Pst>,
{
    fn read(store: Rc<Pst::Store>) -> io::Result<Self> {
        let pst = store.pst();
        let header = pst.header();
        let encoding = header.crypt_method();
        let root = header.root();

        let mut file = pst
            .reader()
            .lock()
            .map_err(|_| MessagingError::FailedToLockFile)?;
        let file = &mut *file;

        let node_btree = <Pst as PstFile>::NodeBTree::read(file, *root.node_btree())?;
        let block_btree = <Pst as PstFile>::BlockBTree::read(file, *root.block_btree())?;

        let mut page_cache = Default::default();
        let node = node_btree.find_entry(
            file,
            u32::from(NID_SEARCH_MANAGEMENT_QUEUE).into(),
            &mut page_cache,
        )?;

        let start = node.parent().map(u32::from).unwrap_or_default();
        if start % SEARCH_UPDATE_SIZE != 0 {
            return Err(MessagingError::InvalidSearchUpdateQueueOffset(start).into());
        }
        let start = usize::try_from(start)
            .map_err(|_| MessagingError::InvalidSearchUpdateQueueOffset(start))?;

        let key = node.data().search_key();
        if u64::from(key) == 0 {
            return Ok(Self {
                updates: Default::default(),
                _phantom: PhantomData,
            });
        }

        let mut page_cache = Default::default();
        let block = block_btree.find_entry(file, key, &mut page_cache)?;
        let tree = DataTree::<Pst>::read(file, encoding, &block)?;
        let mut data = Vec::new();
        let mut block_cache = Default::default();
        let size = tree
            .reader(
                file,
                encoding,
                &block_btree,
                &mut page_cache,
                &mut block_cache,
            )?
            .read_to_end(&mut data)?;

        if size % SEARCH_UPDATE_SIZE as usize != 0 {
            return Err(MessagingError::InvalidSearchUpdateQueueSize(size).into());
        }

        let updates = if size > start {
            let count = (size - start) / SEARCH_UPDATE_SIZE as usize;
            let mut updates = Vec::with_capacity(count);
            let mut cursor = Cursor::new(&data[start..]);
            while let Ok(entry) = SearchUpdate::read(&mut cursor) {
                updates.push(entry);
            }
            updates
        } else {
            Default::default()
        };

        Ok(Self {
            updates,
            _phantom: PhantomData,
        })
    }
}
pub struct UnicodeSearchUpdateQueue {
    inner: SearchUpdateQueueInner<UnicodePstFile>,
}

impl SearchUpdateQueue for UnicodeSearchUpdateQueue {
    fn updates(&self) -> &[SearchUpdate] {
        &self.inner.updates
    }
}

impl SearchUpdateQueueReadWrite<UnicodePstFile> for UnicodeSearchUpdateQueue {
    fn read(store: Rc<UnicodeStore>) -> io::Result<Rc<Self>> {
        let inner = SearchUpdateQueueInner::read(store)?;
        Ok(Rc::new(Self { inner }))
    }
}

pub struct AnsiSearchUpdateQueue {
    inner: SearchUpdateQueueInner<AnsiPstFile>,
}

impl SearchUpdateQueue for AnsiSearchUpdateQueue {
    fn updates(&self) -> &[SearchUpdate] {
        &self.inner.updates
    }
}

impl SearchUpdateQueueReadWrite<AnsiPstFile> for AnsiSearchUpdateQueue {
    fn read(store: Rc<AnsiStore>) -> io::Result<Rc<Self>> {
        let inner = SearchUpdateQueueInner::read(store)?;
        Ok(Rc::new(Self { inner }))
    }
}
