use clap::Parser;
use outlook_pst::{
    ltp::table_context::UnicodeTableContext,
    ndb::{
        header::Header,
        node_id::*,
        page::{RootBTree, UnicodeBlockBTree, UnicodeNodeBTree},
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

        let node = node_btree.find_entry(
            file,
            u64::from(u32::from(NodeId::new(
                NodeIdType::HierarchyTable,
                NID_ROOT_FOLDER.index(),
            )?)),
        )?;
        let hierarchy_table = UnicodeTableContext::read(file, encoding, &block_btree, node)?;
        let context = hierarchy_table.context();

        for row in hierarchy_table.rows_matrix() {
            println!("Row: 0x{:X}", u32::from(row.id()));
            println!("Version: 0x{:X}", row.unique());

            for (column, value) in context.columns().iter().zip(row.columns(context)?) {
                println!(
                    " Column: Property ID: 0x{:04X}, Type: {:?}",
                    column.prop_id(),
                    column.prop_type()
                );

                let Some(value) = value else {
                    println!("  Value: None");
                    continue;
                };

                println!("  Record: {value:?}");

                let value = hierarchy_table.read_column(
                    file,
                    encoding,
                    &block_btree,
                    &value,
                    column.prop_type(),
                )?;
                println!("  Value: {:?}", value);
            }
        }
    }

    Ok(())
}
