use clap::Parser;
use outlook_pst::{ltp::prop_type::PropertyType, messaging::store::UnicodeStore, *};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let pst = UnicodePstFile::read(&args.file).unwrap();
    let store = UnicodeStore::read(&pst).unwrap();
    let properties = store.properties();

    println!("Display Name: {}", properties.display_name().unwrap());
    println!(
        "IPM Subtree: {:?}",
        properties.ipm_sub_tree_entry_id().unwrap()
    );
    println!(
        "Deleted Items: {:?}",
        properties.ipm_wastebasket_entry_id().unwrap()
    );
    println!("Finder: {:?}", properties.finder_entry_id().unwrap());

    for (prop_id, value) in properties.iter() {
        println!(
            " Property ID: 0x{prop_id:04X}, Type: {:?}",
            PropertyType::from(value)
        );
        println!("  Value: {:?}", value);
    }

    Ok(())
}
