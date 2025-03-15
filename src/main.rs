mod commands;
mod entries;
mod media;
mod naming;
mod utils;

use crate::entries::Entry;
use anyhow::{Result, anyhow};
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
    let (dirs, warnings) = valid_dirs(args.dirs)?;
    let entries = Entries::new(dirs, args.shallow, args.filter)?;
    args.cmd.run(entries, warnings)
}

/// Warnings that were encountered while parsing the input paths.
#[derive(Debug)]
pub struct Warnings {
    /// Whether there were missing paths.
    pub missing: bool,
}

fn valid_dirs(dirs: Vec<PathBuf>) -> Result<(Vec<Entry>, Warnings)> {
    let mut dirs = match dirs.is_empty() {
        false => dirs,            // lists files from the given paths,
        true => vec![".".into()], // or the current directory if no paths are given.
    };
    let n = dirs.len();
    dirs.sort_unstable();
    dirs.dedup();
    if n != dirs.len() {
        eprintln!("warning: {} duplicated directories ignored", n - dirs.len());
    }

    let (dirs, missing) = dirs
        .into_iter()
        .map(Entry::try_from)
        .inspect(|res| {
            if let Err(err) = res {
                eprintln!("warning: invalid path: {err}");
            }
        })
        .flatten()
        .partition::<Vec<_>, _>(|entry| entry.is_dir());
    missing
        .iter()
        .for_each(|entry| eprintln!("warning: directory not found: {entry}"));
    if dirs.is_empty() {
        return Err(anyhow!("no valid paths given"));
    }

    let warnings = Warnings {
        missing: !missing.is_empty(),
    };
    Ok((dirs, warnings))
}
