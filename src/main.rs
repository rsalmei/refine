mod commands;
mod entries;
mod medias;
mod utils;

use anyhow::Result;
use clap::Parser;
use commands::Command;
use entries::Input;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine",
    override_usage = "refine <COMMAND> [DIRS]... [FETCH] [OPTIONS]",
)]
pub struct Args {
    #[command(subcommand)]
    cmd: Command,
    #[command(flatten)]
    input: Input,
}

fn main() -> Result<()> {
    utils::install_ctrl_c_handler();

    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    let effective = args.input.try_into()?;
    args.cmd.execute(effective)
}
