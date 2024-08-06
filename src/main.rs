mod commands;
mod entries;
mod utils;

use clap::Parser;
use commands::{dupes, join, list, rebuild, rename, Command, COMMAND};
use entries::{gen_medias, Filters, FILTERS};
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

fn main() {
    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    COMMAND.set(args.cmd).unwrap();
    FILTERS.set(args.filters).unwrap();
    install_ctrlc_handler();

    // lists files from the given paths, or the current directory if no paths were given.
    let cd = args.paths.is_empty().then(|| ".".into());
    let paths = {
        let mut paths = args.paths;
        let len = paths.len();
        paths.sort_unstable();
        paths.dedup();
        if len != paths.len() {
            eprintln!("warning: duplicated paths were ignored");
        }
        paths.into_iter().chain(cd)
    };

    if let Err(err) = match COMMAND.get().unwrap() {
        Command::Dupes(_) => dupes::run(gen_medias(paths, dupes::KIND)),
        Command::Rebuild(_) => rebuild::run(gen_medias(paths, rebuild::KIND)),
        Command::List(_) => list::run(gen_medias(paths, list::KIND)),
        Command::Rename(_) => rename::run(gen_medias(paths, rename::KIND)),
        Command::Join(_) => join::run(gen_medias(paths, join::KIND)),
    } {
        eprintln!("error: {err:?}");
        std::process::exit(1);
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
