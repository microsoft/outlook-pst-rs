use clap::Parser;
use outlook_pst::{
    ltp::{
        heap::{HeapNodeType, UnicodeHeapNode},
        prop_context::UnicodePropertyContext,
        tree::UnicodeHeapTree,
    },
    ndb::{
        block::UnicodeDataTree,
        header::Header,
        node_id::*,
        page::{NodeBTreeEntry, RootBTree, UnicodeBlockBTree, UnicodeNodeBTree},
        root::Root,
    },
    *,
};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let pst = PstFile::read(&args.file).unwrap();
    let header = pst.header();
    let root = header.root();

    {
        let mut file = pst.file().lock().unwrap();
        let file = &mut *file;

        let encoding = header.crypt_method();
        let node_btree = UnicodeNodeBTree::read(file, *root.node_btree())?;
        let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;

        let node = node_btree.find_entry(file, u64::from(u32::from(NID_MESSAGE_STORE)))?;
        let data = node.data();
        let block = block_btree.find_entry(file, u64::from(data))?;
        let heap = UnicodeHeapNode::new(UnicodeDataTree::read(file, encoding, &block)?);
        let header = heap.header(file, encoding, &block_btree)?;

        assert_eq!(header.client_signature(), HeapNodeType::Properties);

        let tree = UnicodeHeapTree::new(heap, header.user_root());
        let tree_header = tree.header(file, encoding, &block_btree)?;

        assert_eq!(tree_header.key_size(), 2);
        assert_eq!(tree_header.entry_size(), 6);

        let prop_context = UnicodePropertyContext::new(tree);
        let properties = prop_context.properties(file, encoding, &block_btree)?;

        for (prop_id, record) in properties {
            println!(
                "Property ID: 0x{prop_id:04X}, Type: {:?}",
                record.prop_type()
            );
            println!(" Record: {:?}", record);

            let value =
                prop_context.read_property(file, encoding, &block_btree, &node_btree, record)?;
            println!(" Value: {:?}", value);
        }
    }

    Ok(())
}
