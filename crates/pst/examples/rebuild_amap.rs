use clap::Parser;
use outlook_pst::*;

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
{
    // This will mark the allocation map as invalid.
    let writer = pst.lock().expect("Failed to lock file for writing");
    std::mem::forget(writer);

    // Since the allocation map is marked as invalid, this will rebuild it.
    let writer = pst.lock().expect("Failed to rebuild allocation map");

    // This will mark the allocation map as valid.
    std::mem::drop(writer);
}
