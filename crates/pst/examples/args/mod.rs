use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about)]
pub struct Args {
    #[clap(default_value = r#"crates/pst/examples/Empty.pst"#)]
    pub file: String,
}
