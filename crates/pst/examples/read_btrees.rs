use clap::Parser;
use outlook_pst::{
    ndb::{
        block_ref::UnicodeBlockRef,
        header::Header,
        page::{
            BTreePage, BTreePageEntry, BlockBTreeEntry, NodeBTreeEntry, UnicodeBlockBTree,
            UnicodeNodeBTree,
        },
        root::Root,
    },
    *,
};
use std::{fs::File, io, iter};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let pst = PstFile::read(&args.file).unwrap();
    let header = pst.header();
    let root = header.root();

    {
        let mut file = pst.file().lock().unwrap();
        output_node_btree(&mut *file, None, *root.node_btree())?;
        output_block_btree(&mut *file, None, *root.block_btree())?;
    }

    Ok(())
}

fn output_node_btree(
    file: &mut File,
    max_level: Option<u8>,
    node_btree: UnicodeBlockRef,
) -> io::Result<()> {
    let node_btree = UnicodeNodeBTree::read(&mut *file, node_btree)?;
    match node_btree {
        UnicodeNodeBTree::Intermediate(page) => {
            let level = page.level();
            let entries = page.entries();

            let indent = max_level
                .map(|max_level| {
                    iter::repeat_n(' ', usize::from(max_level - level)).collect::<String>()
                })
                .unwrap_or_default();
            let max_level = max_level.or(Some(level));

            println!(
                "{indent}Node BTree Level: {level}: Entries: {}",
                entries.len()
            );

            for entry in entries {
                println!("{indent} Key: {:?}", entry.key());
                output_node_btree(file, max_level, entry.block())?;
            }
        }
        UnicodeNodeBTree::Leaf(page, _) => {
            assert_eq!(page.level(), 0);
            let entries = page.entries();

            let indent = max_level
                .map(|max_level| iter::repeat_n(' ', usize::from(max_level)).collect::<String>())
                .unwrap_or_default();

            println!("{indent}Node Page Entries: {}", entries.len());

            for entry in entries {
                println!("{indent} Node: {:?}", entry.node());
                println!("{indent}  Data Block: {:?}", entry.data());
                println!("{indent}  Sub-Node Block: {:?}", entry.sub_node());
                println!("{indent}  Parent Node: {:?}", entry.parent());
            }
        }
    }
    Ok(())
}

fn output_block_btree(
    file: &mut File,
    max_level: Option<u8>,
    block_btree: UnicodeBlockRef,
) -> io::Result<()> {
    let block_btree = UnicodeBlockBTree::read(&mut *file, block_btree)?;
    match block_btree {
        UnicodeBlockBTree::Intermediate(page) => {
            let level = page.level();
            let entries = page.entries();

            let indent = max_level
                .map(|max_level| {
                    iter::repeat_n(' ', usize::from(max_level - level)).collect::<String>()
                })
                .unwrap_or_default();
            let max_level = max_level.or(Some(level));

            println!(
                "{indent}Block BTree Level: {level}: Entries: {}",
                entries.len()
            );

            for entry in entries {
                println!("{indent} Key: {:?}", entry.key());
                output_block_btree(file, max_level, entry.block())?;
            }
        }
        UnicodeBlockBTree::Leaf(page, _) => {
            assert_eq!(page.level(), 0);
            let entries = page.entries();

            let indent = max_level
                .map(|max_level| iter::repeat_n(' ', usize::from(max_level)).collect::<String>())
                .unwrap_or_default();

            println!("{indent}Block Page Entries: {}", entries.len());

            for entry in entries {
                println!("{indent} Block: {:?}", entry.block());
                println!("{indent}  Size: {:?}", entry.size());
                println!("{indent}  Ref-Count: {:?}", entry.ref_count());
            }
        }
    }
    Ok(())
}
