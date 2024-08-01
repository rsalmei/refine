mod commands;
mod entries;
mod utils;

use clap::builder::NonEmptyStringValueParser;
use clap::Parser;
use commands::{dupes, join, list, rebuild, rename, Command};
use entries::gen_medias;
use std::path::PathBuf;
use std::sync::{atomic, Arc, OnceLock};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine")]
struct Args {
    #[command(subcommand)]
    cmd: Command,
    /// Paths to scan.
    #[arg(global = true, help_heading = Some("Global"))]
    paths: Vec<PathBuf>,
    /// Include only these files and directories; checked without extension.
    #[arg(short = 'i', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    include: Option<String>,
    /// Exclude these files and directories; checked without extension.
    #[arg(short = 'x', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    exclude: Option<String>,
    /// Include only these directories.
    #[arg(short = 'I', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    dir_in: Option<String>,
    /// Exclude these directories.
    #[arg(short = 'X', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    dir_ex: Option<String>,
    /// Include only these extensions.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    ext_in: Option<String>,
    /// Exclude these extensions.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    ext_ex: Option<String>,
    /// Do not recurse into subdirectories.
    #[arg(short = 'w', long, global = true, help_heading = Some("Global"))]
    shallow: bool,
}

static ARGS: OnceLock<Args> = OnceLock::new();
fn args() -> &'static Args {
    ARGS.get().unwrap()
}

fn main() {
    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    ARGS.set(Args::parse()).unwrap();
    entries::parse_input_regexes();

    if let Err(err) = ctrlc::set_handler({
        let running = Arc::clone(utils::running_flag());
        move || {
            eprintln!("aborting...");
            running.store(false, atomic::Ordering::Relaxed);
        }
    }) {
        eprintln!("error: set Ctrl-C handler: {err:?}");
    }

    // lists files from the given paths, or the current directory if no paths were given.
    let cd = args().paths.is_empty().then(|| ".".into());
    let paths = args().paths.iter().cloned().chain(cd);

    if let Err(err) = match args().cmd {
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
