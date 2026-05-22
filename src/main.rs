mod commands;
mod entries;
mod fetcher;
mod media;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::RefineCommand;
use yansi::{Color, Paint};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None,
    after_help = "For more information, see https://github.com/rsalmei/refine",
    override_usage = "refine <COMMAND> [DIRS]... [FETCH] [OPTIONS]",
)]
struct CliArgs {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(flatten)]
    Refine(RefineCommand),
}

fn main() -> Result<()> {
    utils::install_ctrl_c_handler();

    println!(
        "{} {}",
        "Refine".paint(Color::Blue).bold(),
        format_args!("v{}", env!("CARGO_PKG_VERSION")).paint(Color::Blue)
    );
    let args = CliArgs::parse();
    match args.command {
        Command::Refine(cmd) => cmd.run(),
    }
}
