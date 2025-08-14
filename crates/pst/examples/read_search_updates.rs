use clap::Parser;

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let store = outlook_pst::open_store(&args.file)?;
    let search_update_queue = store.search_update_queue()?;
    let updates = search_update_queue.updates().to_vec();

    println!("SearchManagementQueue Length: {}", updates.len());

    for (index, update) in updates.into_iter().enumerate() {
        println!(" {index}: {update:?}");
    }

    Ok(())
}
