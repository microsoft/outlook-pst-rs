use clap::Parser;
use outlook_pst::{
    ndb::{
        block::{IntermediateTreeBlock, IntermediateTreeHeader, UnicodeSubNodeTree},
        block_id::UnicodeBlockId,
        block_ref::UnicodeBlockRef,
        header::Header,
        page::{
            BTreeEntry, BTreePage, BTreePageEntry, BlockBTreeEntry, NodeBTreeEntry,
            UnicodeBlockBTree, UnicodeNodeBTree,
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

        output_block_btree(&mut *file, None, *root.block_btree())?;
        println!();

        let block_btree = UnicodeBlockBTree::read(&mut *file, *root.block_btree())?;
        output_node_btree(&mut *file, None, &block_btree, *root.node_btree())?;
    }

    Ok(())
}

fn output_sub_node_tree(
    file: &mut File,
    indent: &str,
    max_level: Option<u8>,
    block_btree: &UnicodeBlockBTree,
    sub_node_btree: UnicodeBlockId,
) -> io::Result<()> {
    let sub_node_block = block_btree.find_entry(file, u64::from(sub_node_btree))?;
    let sub_node_btree = UnicodeSubNodeTree::read(&mut *file, &sub_node_block)?;
    match sub_node_btree {
        UnicodeSubNodeTree::Intermediate(block) => {
            let level = block.header().level();
            let entries = block.entries();

            let sub_node_indent = max_level
                .map(|max_level| {
                    iter::repeat_n(' ', usize::from(max_level - level)).collect::<String>()
                })
                .unwrap_or_default();
            let max_level = max_level.or(Some(level));

            println!(
                "{indent}{sub_node_indent}Sub-Node BTree Level: {level}: Entries: {}",
                entries.len()
            );

            for entry in entries {
                println!("{sub_node_indent} Node: {:?}", entry.node());
                println!("{sub_node_indent} Block: {:?}", entry.block());
                output_sub_node_tree(file, indent, max_level, block_btree, entry.block())?;
            }
        }
        UnicodeSubNodeTree::Leaf(block) => {
            assert_eq!(block.header().level(), 0);
            let entries = block.entries();

            let sub_node_indent = max_level
                .map(|max_level| iter::repeat_n(' ', usize::from(max_level)).collect::<String>())
                .unwrap_or_default();

            println!(
                "{indent}{sub_node_indent}Sub-Node Block Entries: {}",
                entries.len()
            );

            for entry in entries {
                println!("{indent}{sub_node_indent} Node: {:?}", entry.node());

                println!("{indent}{sub_node_indent} Block: {:?}", entry.block());
                let sub_node_block = block_btree.find_entry(file, u64::from(entry.block()))?;
                println!(
                    "{indent}{sub_node_indent}  BlockRef: {:?}",
                    sub_node_block.block()
                );

                if let Some(sub_node) = entry.sub_node() {
                    let indent = format!("{indent}{sub_node_indent}  ");
                    println!("{indent}Sub-Node Block: {:?}", sub_node);
                    output_sub_node_tree(file, &indent, None, block_btree, sub_node)?;
                } else {
                    println!("{indent}{sub_node_indent}  Sub-Node Block: None");
                }
            }
        }
    }
    Ok(())
}

fn output_node_btree(
    file: &mut File,
    max_level: Option<u8>,
    block_btree: &UnicodeBlockBTree,
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
                output_node_btree(file, max_level, block_btree, entry.block())?;
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
                if let Some(sub_node) = entry.sub_node() {
                    println!("{indent}  Sub-Node Block: {:?}", sub_node);
                    let indent = format!("{indent}   ");
                    output_sub_node_tree(file, &indent, None, block_btree, sub_node)?;
                } else {
                    println!("{indent}  Sub-Node Block: None");
                }

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
