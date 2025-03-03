//! ## [Message Store](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/aa0539bd-e7bf-4cec-8bde-0b87c2a86baf)

use std::{collections::BTreeMap, io};

use super::*;
use crate::{
    ltp::{
        heap::{AnsiHeapNode, UnicodeHeapNode},
        prop_context::{AnsiPropertyContext, PropertyValue, UnicodePropertyContext},
        tree::{AnsiHeapTree, UnicodeHeapTree},
    },
    ndb::{
        block::{AnsiDataTree, UnicodeDataTree},
        header::Header,
        node_id::NID_MESSAGE_STORE,
        page::{
            AnsiBlockBTree, AnsiNodeBTree, NodeBTreeEntry, RootBTree, UnicodeBlockBTree,
            UnicodeNodeBTree,
        },
        root::Root,
    },
    AnsiPstFile, UnicodePstFile,
};

pub struct UnicodeStore<'a> {
    pst: &'a UnicodePstFile,
    properties: BTreeMap<u16, PropertyValue>,
}

impl<'a> UnicodeStore<'a> {
    pub fn pst(&self) -> &UnicodePstFile {
        self.pst
    }

    pub fn read(pst: &'a UnicodePstFile) -> io::Result<Self> {
        let header = pst.header();
        let root = header.root();

        let properties = {
            let mut file = pst
                .file()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = UnicodeNodeBTree::read(file, *root.node_btree())?;
            let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u64::from(u32::from(NID_MESSAGE_STORE)))?;
            let data = node.data();
            let block = block_btree.find_entry(file, u64::from(data))?;
            let heap = UnicodeHeapNode::new(UnicodeDataTree::read(file, encoding, &block)?);
            let header = heap.header(file, encoding, &block_btree)?;

            let tree = UnicodeHeapTree::new(heap, header.user_root());
            let prop_context = UnicodePropertyContext::new(node, tree);
            prop_context
                .properties(file, encoding, &block_btree)?
                .into_iter()
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()
        }?;

        Ok(Self { pst, properties })
    }

    pub fn properties(&self) -> &BTreeMap<u16, PropertyValue> {
        &self.properties
    }
}

pub struct AnsiStore<'a> {
    pst: &'a AnsiPstFile,
    properties: BTreeMap<u16, PropertyValue>,
}

impl<'a> AnsiStore<'a> {
    pub fn pst(&self) -> &AnsiPstFile {
        self.pst
    }

    pub fn read(pst: &'a AnsiPstFile) -> io::Result<Self> {
        let header = pst.header();
        let root = header.root();

        let properties = {
            let mut file = pst
                .file()
                .lock()
                .map_err(|_| MessagingError::FailedToLockFile)?;
            let file = &mut *file;

            let encoding = header.crypt_method();
            let node_btree = AnsiNodeBTree::read(file, *root.node_btree())?;
            let block_btree = AnsiBlockBTree::read(file, *root.block_btree())?;

            let node = node_btree.find_entry(file, u32::from(NID_MESSAGE_STORE))?;
            let data = node.data();
            let block = block_btree.find_entry(file, u32::from(data))?;
            let heap = AnsiHeapNode::new(AnsiDataTree::read(file, encoding, &block)?);
            let header = heap.header(file, encoding, &block_btree)?;

            let tree = AnsiHeapTree::new(heap, header.user_root());
            let prop_context = AnsiPropertyContext::new(node, tree);
            prop_context
                .properties(file, encoding, &block_btree)?
                .into_iter()
                .map(|(prop_id, record)| {
                    prop_context
                        .read_property(file, encoding, &block_btree, record)
                        .map(|value| (prop_id, value))
                })
                .collect::<io::Result<BTreeMap<_, _>>>()
        }?;

        Ok(Self { pst, properties })
    }

    pub fn properties(&self) -> &BTreeMap<u16, PropertyValue> {
        &self.properties
    }
}
