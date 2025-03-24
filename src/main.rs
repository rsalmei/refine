mod commands;
mod entries;
mod media;
mod utils;

use anyhow::Result;
use clap::Parser;
use commands::Command;
use entries::input::Input;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine",
    override_usage = "refine <COMMAND> [DIRS]... [FETCH] [OPTIONS]",
)]
pub struct Args {
    #[command(subcommand)]
    cmd: Command,
    #[command(flatten)]
    input: Input,
    /// Override the called command to just view the filtered input entries.
    #[arg(long, global = true, help_heading = Some("Fetch"))]
    view: bool,
}

fn main() -> Result<()> {
    utils::install_ctrl_c_handler();

    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    let (fetcher, warnings) = args.input.try_into()?;
    match args.view {
        false => args.cmd.run(fetcher, warnings),
        true => {
            args.cmd.view(fetcher);
            Ok(())
        }
    }
}
