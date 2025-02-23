mod commands;
mod utils;

use anyhow::Result;
use clap::Parser;
use commands::Command;
use utils::InputSpec;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine")]
pub struct Args {
    #[command(flatten)]
    spec: InputSpec,
    #[command(subcommand)]
    cmd: Command,
}

fn main() -> Result<()> {
    utils::install_ctrl_c_handler();

    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    let entries = args.spec.try_into()?;
    args.cmd.run(entries)
}
