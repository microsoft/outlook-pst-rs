use clap::Parser;
use outlook_pst::{
    ndb::{block_id::BlockId, byte_index::ByteIndex},
    *,
};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;

    if let Ok(mut pst) = UnicodePstFile::open(&args.file) {
        rebuild_amap(&mut pst);
    } else {
        let mut pst = AnsiPstFile::open(&args.file)?;
        rebuild_amap(&mut pst);
    }

    Ok(())
}

fn rebuild_amap<Pst>(pst: &mut Pst)
where
    Pst: PstFile,
    u64: From<<<Pst as PstFile>::BlockId as BlockId>::Index>
        + From<<<Pst as PstFile>::ByteIndex as ByteIndex>::Index>,
{
    // This will mark the allocation map as invalid.
    pst.start_write()
        .expect("Failed to start write transaction");

    // Since the allocation map is marked as invalid, this will rebuild it.
    pst.start_write()
        .expect("Failed to rebuild allocation map");

    // This will mark the allocation map as valid.
    pst.finish_write()
        .expect("Failed to finish write transaction");
}
