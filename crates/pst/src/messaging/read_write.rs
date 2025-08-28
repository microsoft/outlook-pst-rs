#![allow(dead_code)]

use crate::*;
use std::io::{self, Read, Write};

pub trait StoreKeyReadWrite: Sized {
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait StoreReadWrite<Pst>: Store + Sized
where
    Pst: PstFile,
{
    fn pst(&self) -> &Pst;
    fn node_btree(&self) -> &PstFileReadWriteNodeBTree<Pst>;
    fn block_btree(&self) -> &PstFileReadWriteBlockBTree<Pst>;
}

pub trait FolderReadWrite<Pst>: Folder + Sized
where
    Pst: PstFile,
{
    fn read(store: Rc<Pst::Store>, entry_id: &EntryId) -> io::Result<Rc<Self>>;
}

pub trait MessageReadWrite<Pst>: Message + Sized
where
    Pst: PstFile,
{
    fn read(
        store: Rc<Pst::Store>,
        entry_id: &EntryId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>>;
    fn read_embedded(
        store: Rc<Pst::Store>,
        node: Pst::NodeBTreeEntry,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>>;
    fn pst_store(&self) -> &Rc<Pst::Store>;
    fn sub_nodes(&self) -> &MessageSubNodes<Pst>;
}

pub trait AttachmentReadWrite<Pst>: Sized
where
    Pst: PstFile,
{
    fn read(
        message: Rc<Pst::Message>,
        sub_node: NodeId,
        prop_ids: Option<&[u16]>,
    ) -> io::Result<Rc<Self>>;
}

pub trait NamedPropReadWrite: Sized {
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait NamedPropertyMapReadWrite<Pst>: NamedPropertyMap + Sized
where
    Pst: PstFile,
{
    fn read(store: Rc<Pst::Store>) -> io::Result<Rc<Self>>;
}

pub trait SearchReadWrite: Sized {
    fn read(f: &mut dyn Read) -> io::Result<Self>;
    fn write(&self, f: &mut dyn Write) -> io::Result<()>;
}

pub trait SearchUpdateQueueReadWrite<Pst>: SearchUpdateQueue + Sized
where
    Pst: PstFile,
{
    fn read(store: Rc<Pst::Store>) -> io::Result<Rc<Self>>;
}
