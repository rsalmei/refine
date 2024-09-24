mod commands;
mod entries;
mod utils;

use crate::entries::find_entries;
use anyhow::Result;
use clap::Parser;
use commands::{Command, Refine};
use entries::Filters;
use std::path::PathBuf;
use std::sync::{atomic, Arc};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine")]
pub struct Args {
    /// Paths to scan.
    #[arg(global = true, help_heading = Some("Global"))]
    pub paths: Vec<PathBuf>,
    #[command(subcommand)]
    pub cmd: Command,
    #[command(flatten)]
    pub filters: Filters,
}

fn main() -> Result<()> {
    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    install_ctrlc_handler();

    let options = {
        let mut paths = args.paths;
        let len = paths.len();
        paths.sort_unstable();
        paths.dedup();
        if len != paths.len() {
            eprintln!("warning: duplicated paths were ignored");
        }
        // lists files from the given paths, or the current directory if no paths were given.
        let cd = paths.is_empty().then(|| ".".into());
        (paths.into_iter().chain(cd), args.filters)
    };

    match args.cmd {
        Command::Dupes(cmd) => run(cmd, options),
        Command::Rebuild(cmd) => run(cmd, options),
        Command::List(cmd) => run(cmd, options),
        Command::Rename(cmd) => run(cmd, options),
        Command::Join(cmd) => run(cmd, options),
    }
}

fn run<R: Refine>(
    cmd: R,
    (paths, filters): (impl Iterator<Item = PathBuf>, Filters),
) -> Result<()> {
    cmd.refine(gen_medias(find_entries(filters, paths, R::entry_kind())?))
}

fn gen_medias<T>(entries: impl Iterator<Item = PathBuf>) -> Vec<T>
where
    T: TryFrom<PathBuf, Error: std::fmt::Display>,
{
    entries
        .map(|path| T::try_from(path))
        .inspect(|res| {
            if let Err(err) = res {
                eprintln!("error: load media: {err}");
            }
        })
        .flatten()
        .collect()
}

fn install_ctrlc_handler() {
    if let Err(err) = ctrlc::set_handler({
        let running = Arc::clone(utils::running_flag());
        move || {
            eprintln!("aborting...");
            running.store(false, atomic::Ordering::Relaxed);
        }
    }) {
        eprintln!("error: set Ctrl-C handler: {err:?}");
    }
}
