use clap::Parser;
use outlook_pst::{ndb::page::PageTrailer, *};
use std::fmt::Debug;

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    if let Ok(pst) = UnicodePstFile::open(&args.file) {
        read_density_list(pst);
    } else {
        let pst = AnsiPstFile::open(&args.file)?;
        read_density_list(pst);
    }

    Ok(())
}

fn read_density_list<Pst>(pst: Pst)
where
    Pst: PstFile,
    <Pst as PstFile>::BlockId: Debug,
    <Pst as PstFile>::ByteIndex: Debug,
{
    let density_list = pst.density_list();
    let density_list = match density_list {
        Ok(density_list) => density_list,
        Err(err) => {
            println!("Error: {err:?}");
            return;
        }
    };

    let backfill_complete = density_list.backfill_complete();
    let current_page = density_list.current_page();
    let entries = density_list.entries();

    println!("Backfill Complete: {backfill_complete}");
    println!("Current Page: {current_page}");
    println!("Density List Entries: {entries:?}");

    let trailer = density_list.trailer();

    let page_type = trailer.page_type();
    let signature = trailer.signature();
    let crc = trailer.crc();
    let block_id = trailer.block_id();

    println!("Page Type: {page_type:?}");
    println!("Page Signature: 0x{signature:0x}");
    println!("Page CRC: 0x{crc:08x}");
    println!("Block ID: {block_id:?}");
}
