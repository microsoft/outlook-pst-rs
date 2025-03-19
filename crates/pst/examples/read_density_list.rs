use clap::Parser;
use outlook_pst::{
    ndb::page::{DensityListPage, PageTrailer},
    *,
};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    if let Ok(pst) = UnicodePstFile::read(&args.file) {
        let density_list = pst.density_list();
        match density_list {
            Ok(density_list) => read_density_list(density_list),
            Err(err) => {
                println!("Error: {err:?}");
                return Ok(());
            }
        };
    } else {
        let pst = AnsiPstFile::read(&args.file)?;
        let density_list = pst.density_list();
        match density_list {
            Ok(density_list) => read_density_list(density_list),
            Err(err) => {
                println!("Error: {err:?}");
                return Ok(());
            }
        };
    }

    Ok(())
}

fn read_density_list<T: PageTrailer>(density_list: &dyn DensityListPage<Trailer = T>) {
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
