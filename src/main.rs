mod commands;
mod entries;
mod media;
mod naming;
mod utils;

use anyhow::Result;
use clap::Parser;
use commands::Command;
use entries::{Entries, Filter};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine")]
pub struct Args {
    /// Directories to scan.
    #[arg(global = true, help_heading = Some("Global"))]
    dirs: Vec<PathBuf>,
    /// Do not recurse into subdirectories.
    #[arg(short = 'w', long, global = true, help_heading = Some("Global"))]
    shallow: bool,
    #[command(flatten)]
    filter: Filter,
    #[command(subcommand)]
    cmd: Command,
}

fn main() -> Result<()> {
    utils::install_ctrl_c_handler();

    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    let entries = Entries::new(args.dirs, args.shallow, args.filter)?;
    args.cmd.run(entries)
}
