use clap::Parser;
use outlook_pst::messaging::named_prop::*;

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let store = outlook_pst::open_store(&args.file)?;
    let named_props = store.named_property_map()?;
    let properties = named_props.properties();

    for entry in properties.stream_entry()? {
        let prop_id = entry.prop_id();
        println!("Named Property ID: 0x{prop_id:04X}");

        let guid = entry.guid();
        println!(" GUID Index: {guid:?}");
        match guid {
            NamedPropertyGuid::None => {}
            NamedPropertyGuid::Mapi => {
                println!(" PS_MAPI: {PS_MAPI:?}");
            }
            NamedPropertyGuid::PublicStrings => {
                println!(" PS_PUBLIC_STRINGS: {PS_PUBLIC_STRINGS:?}");
            }
            NamedPropertyGuid::GuidIndex(index) => {
                let guid = properties
                    .lookup_guid(
                        NamedPropertyGuid::try_from(index)
                            .expect("Failed to convert index to NamedPropertyGuid"),
                    )
                    .expect("Failed to lookup GUID");
                println!(" Other: {guid:?}");
            }
        }

        match entry.id() {
            NamedPropertyId::Number(id) => {
                println!(" Number: 0x{id:08X}");
            }
            NamedPropertyId::StringOffset(index) => {
                print!(" String[0x{index:08X}]: ");
                let string_entry = properties
                    .lookup_string(index)
                    .expect("Failed to lookup string")
                    .to_string();
                println!("{string_entry:?}");
            }
        }
    }

    Ok(())
}
