mod commands;
mod entries;
mod utils;

use anyhow::Result;
use clap::Parser;
use commands::Command;
use entries::{Entries, Filters};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine")]
pub struct Args {
    /// Directories to scan.
    #[arg(global = true, help_heading = Some("Global"))]
    pub dirs: Vec<PathBuf>,
    #[command(subcommand)]
    pub cmd: Command,
    #[command(flatten)]
    pub filters: Filters,
}

fn main() -> Result<()> {
    utils::install_ctrl_c_handler();

    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    let entries = {
        // lists files from the given paths, or the current directory if no paths were given.
        let dirs = args.dirs.is_empty().then(|| vec![".".into()]);
        Entries::new(dirs.unwrap_or(args.dirs), args.filters)?
    };
    args.cmd.run(entries)
}
