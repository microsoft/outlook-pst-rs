use clap::Parser;
use outlook_pst::{ltp::prop_type::PropertyType, messaging::store::UnicodeStore, *};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let pst = UnicodePstFile::read(&args.file).unwrap();
    let store = UnicodeStore::read(&pst).unwrap();
    let properties = store.properties();

    for (prop_id, value) in properties {
        println!(
            "Property ID: 0x{prop_id:04X}, Type: {:?}",
            PropertyType::from(value)
        );
        println!(" Value: {:?}", value);
    }

    Ok(())
}
