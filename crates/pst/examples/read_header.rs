use outlook_pst::{
    ndb::{
        BTreePage, BTreePageEntry, BlockBTreeEntry, Header, NodeBTreeEntry, Root,
        UnicodeBlockBTree, UnicodeBlockRef, UnicodeNodeBTree,
    },
    *,
};
use std::{fs::File, io, iter};

fn main() -> anyhow::Result<()> {
    let pst = PstFile::read(r#"crates/pst/examples/Empty.pst"#).unwrap();
    let header = pst.header();
    let version = header.version();

    println!("File Version: {version:?}");

    let root = header.root();
    let file_eof_index = root.file_eof_index();
    let amap_last_index = root.amap_last_index();
    let amap_free_size = root.amap_free_size();
    let pmap_free_size = root.pmap_free_size();
    let node_btree = root.node_btree();
    let block_btree = root.block_btree();
    let amap_is_valid = root.amap_is_valid();

    println!("File EOF Index: {file_eof_index:?}");
    println!("AMAP Last Index: {amap_last_index:?}");
    println!("AMAP Free Size: {amap_free_size:?}");
    println!("PMAP Free Size: {pmap_free_size:?}");
    println!("NBT BlockRef: {node_btree:?}");
    println!("BBT BlockRef: {block_btree:?}");
    println!("AMAP Valid: {amap_is_valid:?}");

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
        ndb::UnicodeNodeBTree::Intermediate(page) => {
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
        ndb::UnicodeNodeBTree::Leaf(page, _) => {
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
        ndb::UnicodeBlockBTree::Intermediate(page) => {
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
        ndb::UnicodeBlockBTree::Leaf(page, _) => {
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
