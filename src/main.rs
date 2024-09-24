mod commands;
mod entries;
mod utils;

use anyhow::Result;
use clap::Parser;
use commands::{run, Command};
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
        // lists files from the given paths, or the current directory if no paths were given.
        let paths = args.paths.is_empty().then(|| vec![".".into()]);
        (paths.unwrap_or(args.paths), args.filters)
    };

    match args.cmd {
        Command::Dupes(cmd) => run(cmd, options),
        Command::Rebuild(cmd) => run(cmd, options),
        Command::List(cmd) => run(cmd, options),
        Command::Rename(cmd) => run(cmd, options),
        Command::Join(cmd) => run(cmd, options),
    }
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
