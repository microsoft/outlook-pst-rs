use clap::Parser;
use outlook_pst::{ltp::prop_type::PropertyType, messaging::store::StoreProperties};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let store = outlook_pst::open_store(&args.file)?;
    let properties = store.properties();
    read_store_props(properties)
}

fn read_store_props(properties: &StoreProperties) -> anyhow::Result<()> {
    println!("Display Name: {}", properties.display_name()?);
    println!("IPM Subtree: {:?}", properties.ipm_sub_tree_entry_id()?);
    println!(
        "Deleted Items: {:?}",
        properties.ipm_wastebasket_entry_id()?
    );
    println!("Finder: {:?}", properties.finder_entry_id()?);

    for (prop_id, value) in properties.iter() {
        println!(
            " Property ID: 0x{prop_id:04X}, Type: {:?}",
            PropertyType::from(value)
        );
        println!("  Value: {value:?}");
    }

    Ok(())
}
