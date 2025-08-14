use clap::Parser;

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let store = outlook_pst::open_store(&args.file)?;
    let hierarchy_table = store.root_hierarchy_table()?;
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

            let value = hierarchy_table.read_column(&value, column.prop_type())?;
            println!("  Value: {value:?}");
        }
    }

    Ok(())
}
