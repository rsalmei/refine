mod commands;
mod entries;
mod medias;
mod utils;

use anyhow::Result;
use clap::Parser;
use commands::Command;
use entries::{Entry, Input};
use utils::natural_cmp;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine",
    override_usage = "refine <COMMAND> [DIRS]... [FETCH] [OPTIONS]",
)]
pub struct Args {
    #[command(subcommand)]
    cmd: Command,
    /// Just show the entries that would be processed, without running the command.
    #[arg(long, global = true)]
    show: bool,
    #[command(flatten)]
    input: Input,
}

fn main() -> Result<()> {
    utils::install_ctrl_c_handler();

    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    let (fetcher, info) = args.input.try_into()?;
    match args.show {
        false => args.cmd.run(fetcher, info),
        true => {
            let mode = args.cmd.traversal_mode();
            show(fetcher.fetch(mode));
            Ok(())
        }
    }
}

fn show(entries: impl Iterator<Item = Entry>) {
    println!("\nentries this command will process:\n");
    let mut entries = entries.collect::<Vec<_>>();
    entries.sort_unstable_by(|e, f| natural_cmp(e.to_str(), f.to_str()));
    entries.iter().for_each(|e| println!("{e}"));
    match entries.len() {
        0 => println!("no entries found"),
        n => println!("\ntotal entries: {n}"),
    }
}
