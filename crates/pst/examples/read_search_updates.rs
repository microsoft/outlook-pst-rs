use clap::Parser;
use outlook_pst::{
    messaging::search::UnicodeSearchUpdateQueue,
    ndb::{
        header::Header,
        node_id::NID_SEARCH_MANAGEMENT_QUEUE,
        page::{UnicodeBlockBTree, UnicodeNodeBTree},
        root::Root,
    },
    *,
};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let pst = UnicodePstFile::open(&args.file).unwrap();
    let header = pst.header();
    let root = header.root();

    let updates = {
        let mut file = pst.reader().lock().unwrap();
        let file = &mut *file;

        let node_btree = UnicodeNodeBTree::read(file, *root.node_btree())?;
        let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;

        let node =
            node_btree.find_entry(file, u64::from(u32::from(NID_SEARCH_MANAGEMENT_QUEUE)))?;
        let search_update_queue =
            UnicodeSearchUpdateQueue::read(file, header.crypt_method(), &block_btree, node)?;
        search_update_queue.updates().to_vec()
    };

    println!("SearchManagementQueue Length: {}", updates.len());

    for (index, update) in updates.into_iter().enumerate() {
        println!(" {index}: {update:?}");
    }

    Ok(())
}
