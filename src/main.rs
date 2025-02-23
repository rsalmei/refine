mod commands;
mod utils;

use anyhow::Result;
use clap::Parser;
use commands::Command;
use std::path::PathBuf;
use utils::{Entries, Filters};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine")]
pub struct Args {
    /// Directories to scan.
    #[arg(global = true, help_heading = Some("Global"))]
    dirs: Vec<PathBuf>,
    /// Do not recurse into subdirectories.
    #[arg(short = 'w', long, global = true, help_heading = Some("Global"))]
    shallow: bool,
    #[command(subcommand)]
    cmd: Command,
    #[command(flatten)]
    filters: Filters,
}

fn main() -> Result<()> {
    utils::install_ctrl_c_handler();

    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    let dirs = match args.dirs.is_empty() {
        false => args.dirs,       // lists files from the given paths,
        true => vec![".".into()], // or the current directory if no paths are given.
    };
    let entries = Entries::new(dirs, args.shallow, args.filters)?;
    args.cmd.run(entries)
}
