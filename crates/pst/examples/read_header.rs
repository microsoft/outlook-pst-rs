use clap::Parser;
use outlook_pst::{
    ndb::{header::Header, root::Root},
    *,
};
use std::fmt::Debug;

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;

    if let Ok(pst) = UnicodePstFile::open(&args.file) {
        read_header(&pst);
    } else {
        let pst = AnsiPstFile::open(&args.file)?;
        read_header(&pst);
    }

    Ok(())
}

fn read_header<Pst>(pst: &Pst)
where
    Pst: PstFile,
    <Pst as PstFile>::ByteIndex: Debug,
    <Pst as PstFile>::BlockRef: Debug,
{
    let header = pst.header();
    let version = header.version();

    println!("File Version: {version:?}");

    let root = header.root();
    let file_eof_index = root.file_eof_index();
    let amap_last_index = root.amap_last_index();
    let amap_free_size = root.amap_free_size();
    let pmap_free_size = root.pmap_free_size();
    let node_btree = root.node_btree();
    let block_btree = root.block_btree();
    let amap_is_valid = root.amap_is_valid();

    println!("File EOF Index: {file_eof_index:?}");
    println!("AMAP Last Index: {amap_last_index:?}");
    println!("AMAP Free Size: {amap_free_size:?}");
    println!("PMAP Free Size: {pmap_free_size:?}");
    println!("NBT BlockRef: {node_btree:?}");
    println!("BBT BlockRef: {block_btree:?}");
    println!("AMAP Valid: {amap_is_valid:?}");
}
