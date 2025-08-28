use clap::Parser;
use outlook_pst::{
    ndb::{
        block::{
            Block, BlockTrailer, IntermediateDataTreeEntry, IntermediateTreeBlock,
            IntermediateTreeHeader, UnicodeDataTree, UnicodeSubNodeTree,
        },
        block_id::{BlockId, UnicodeBlockId},
        block_ref::UnicodePageRef,
        header::{Header, NdbCryptMethod},
        node_id::NodeId,
        page::{
            BTreeEntry, BTreePage, BTreePageEntry, BlockBTreeEntry, NodeBTreeEntry,
            UnicodeBlockBTree, UnicodeNodeBTree,
        },
        root::Root,
    },
    *,
};
use std::{io, iter};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let pst = UnicodePstFile::open(&args.file).expect("Failed to open PST file");
    let header = pst.header();
    let root = header.root();

    {
        let mut file = pst.reader().lock().expect("Failed to lock reader");
        let file = &mut *file;

        output_block_btree(file, None, *root.block_btree())?;
        println!();

        let block_btree = UnicodeBlockBTree::read(file, *root.block_btree())?;
        output_node_btree(
            file,
            header.crypt_method(),
            None,
            &block_btree,
            *root.node_btree(),
        )?;
    }

    Ok(())
}

fn output_data_tree<R: PstReader>(
    file: &mut R,
    encoding: NdbCryptMethod,
    indent: &str,
    max_level: Option<u8>,
    block_btree: &UnicodeBlockBTree,
    node_data_tree: UnicodeBlockId,
) -> io::Result<()> {
    let mut page_cache = Default::default();
    let data_block = block_btree.find_entry(file, node_data_tree.search_key(), &mut page_cache)?;
    let data_tree = UnicodeDataTree::read(&mut *file, encoding, &data_block)?;
    match data_tree {
        UnicodeDataTree::Intermediate(block) => {
            let level = block.header().level();
            let entries = block.entries();

            let sub_block_indent = max_level
                .map(|max_level| {
                    iter::repeat_n(' ', usize::from(max_level - level)).collect::<String>()
                })
                .unwrap_or_default();
            let max_level = max_level.or(Some(level));

            println!(
                "{indent}{sub_block_indent}Data Tree Level: {level}: Entries: {}",
                entries.len()
            );

            println!(
                "{indent}{sub_block_indent}Total Size: 0x{:X}",
                block.header().total_size()
            );

            for entry in entries {
                println!("{indent}{sub_block_indent} Block: {:?}", entry.block());
                output_data_tree(
                    file,
                    encoding,
                    indent,
                    max_level,
                    block_btree,
                    entry.block(),
                )?;
            }
        }
        UnicodeDataTree::Leaf(block) => {
            let sub_block_indent = max_level
                .map(|max_level| iter::repeat_n(' ', usize::from(max_level)).collect::<String>())
                .unwrap_or_default();

            println!(
                "{indent}{sub_block_indent}Data Block: {:?}",
                block.trailer().block_id()
            );
            println!("{indent}{sub_block_indent}Size: 0x{:X}", block.data().len());
        }
    }
    Ok(())
}

fn output_sub_node_tree<R: PstReader>(
    file: &mut R,
    encoding: NdbCryptMethod,
    indent: &str,
    max_level: Option<u8>,
    block_btree: &UnicodeBlockBTree,
    sub_node_btree: UnicodeBlockId,
) -> io::Result<()> {
    let mut page_cache = Default::default();
    let sub_node_block =
        block_btree.find_entry(file, sub_node_btree.search_key(), &mut page_cache)?;
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
                output_sub_node_tree(
                    file,
                    encoding,
                    indent,
                    max_level,
                    block_btree,
                    entry.block(),
                )?;
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

                let data_tree_indent = format!("{indent}{sub_node_indent} ");
                output_data_tree(
                    file,
                    encoding,
                    &data_tree_indent,
                    None,
                    block_btree,
                    entry.block(),
                )?;

                let sub_node_block =
                    block_btree.find_entry(file, entry.block().search_key(), &mut page_cache)?;
                println!(
                    "{indent}{sub_node_indent}  PageRef: {:?}",
                    sub_node_block.block()
                );

                if let Some(sub_node) = entry.sub_node() {
                    let indent = format!("{indent}{sub_node_indent}  ");
                    println!("{indent}Sub-Node Block: {sub_node:?}");
                    output_sub_node_tree(file, encoding, &indent, None, block_btree, sub_node)?;
                } else {
                    println!("{indent}{sub_node_indent}  Sub-Node Block: None");
                }
            }
        }
    }
    Ok(())
}

fn output_node_btree<R: PstReader>(
    file: &mut R,
    encoding: NdbCryptMethod,
    max_level: Option<u8>,
    block_btree: &UnicodeBlockBTree,
    node_btree: UnicodePageRef,
) -> io::Result<()> {
    let node_btree = UnicodeNodeBTree::read(&mut *file, node_btree)?;
    match node_btree {
        UnicodeNodeBTree::Intermediate(page, ..) => {
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
                let Ok(key) = u32::try_from(entry.key()).map(NodeId::from) else {
                    println!("{indent} Invalid Key: 0x{:X}", entry.key());
                    continue;
                };
                println!("{indent} Key: {key:?}");
                output_node_btree(file, encoding, max_level, block_btree, entry.block())?;
            }
        }
        UnicodeNodeBTree::Leaf(page) => {
            assert_eq!(page.level(), 0);
            let entries = page.entries();

            let indent = max_level
                .map(|max_level| iter::repeat_n(' ', usize::from(max_level)).collect::<String>())
                .unwrap_or_default();

            println!("{indent}Node Page Entries: {}", entries.len());

            for entry in entries {
                println!("{indent} Node: {:?}", entry.node());
                let indent = format!("{indent}  ");
                if entry.data().search_key() == 0 {
                    println!("{indent}Data Block: {:?}", entry.data());
                } else {
                    output_data_tree(file, encoding, &indent, None, block_btree, entry.data())?;
                }

                if let Some(sub_node) = entry.sub_node() {
                    println!("{indent}Sub-Node Block: {sub_node:?}");
                    let indent = format!("{indent} ");
                    output_sub_node_tree(file, encoding, &indent, None, block_btree, sub_node)?;
                } else {
                    println!("{indent}Sub-Node Block: None");
                }

                println!("{indent}Parent Node: {:?}", entry.parent());
            }
        }
    }
    Ok(())
}

fn output_block_btree<R: PstReader>(
    file: &mut R,
    max_level: Option<u8>,
    block_btree: UnicodePageRef,
) -> io::Result<()> {
    let block_btree = UnicodeBlockBTree::read(&mut *file, block_btree)?;
    match block_btree {
        UnicodeBlockBTree::Intermediate(page, ..) => {
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
        UnicodeBlockBTree::Leaf(page) => {
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
