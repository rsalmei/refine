mod dupes;
mod join;
mod list;
mod rebuild;
mod rename;
mod utils;

use clap::builder::NonEmptyStringValueParser;
use clap::{Parser, Subcommand};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::{atomic, Arc, OnceLock};
use std::{fmt, iter};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine")]
struct Args {
    #[command(subcommand)]
    cmd: Command,
    /// Paths to scan.
    #[arg(global = true, help_heading = Some("Global"))]
    paths: Vec<PathBuf>,
    /// Include only these filenames; without extension, case-insensitive.
    #[arg(short = 'i', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    include: Option<String>,
    /// Exclude these filenames; without extension, case-insensitive.
    #[arg(short = 'x', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    exclude: Option<String>,
    /// Include only these extensions; case-insensitive.
    #[arg(short = 'I', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    ext_in: Option<String>,
    /// Exclude these extensions; case-insensitive.
    #[arg(short = 'X', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    ext_ex: Option<String>,
    /// Include only these subdirectories; case-insensitive.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    dir_in: Option<String>,
    /// Exclude these subdirectories; case-insensitive.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    dir_ex: Option<String>,
    /// Do not recurse into subdirectories.
    #[arg(short = 'w', long, global = true, help_heading = Some("Global"))]
    shallow: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Find possibly duplicated files by both size and filename.
    Dupes(dupes::Dupes),
    /// Rebuild the filenames of media collections intelligently.
    Rebuild(rebuild::Rebuild),
    /// List files from the given paths.
    List(list::List),
    /// Rename files in batch, according to the given rules.
    Rename(rename::Rename),
    /// Join all files into the same directory.
    Join(join::Join),
}

static RE_IN: OnceLock<Regex> = OnceLock::new();
static RE_EX: OnceLock<Regex> = OnceLock::new();
static RE_EXT_IN: OnceLock<Regex> = OnceLock::new();
static RE_EXT_EX: OnceLock<Regex> = OnceLock::new();
static RE_DIR_IN: OnceLock<Regex> = OnceLock::new();
static RE_DIR_EX: OnceLock<Regex> = OnceLock::new();

static ARGS: OnceLock<Args> = OnceLock::new();
fn args() -> &'static Args {
    ARGS.get().unwrap()
}

fn main() {
    ARGS.set(Args::parse()).unwrap();
    println!("Refine v{}", env!("CARGO_PKG_VERSION"));
    utils::set_re(&args().include, &RE_IN, "include");
    utils::set_re(&args().exclude, &RE_EX, "exclude");
    utils::set_re(&args().ext_in, &RE_EXT_IN, "ext_in");
    utils::set_re(&args().ext_ex, &RE_EXT_EX, "ext_ex");
    utils::set_re(&args().dir_in, &RE_DIR_IN, "dir_in");
    utils::set_re(&args().dir_ex, &RE_DIR_EX, "dir_ex");

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
    let files = args().paths.iter().cloned().chain(cd).flat_map(entries);

    if let Err(err) = match args().cmd {
        Command::Dupes(_) => dupes::run(gen_medias(files)),
        Command::Rebuild(_) => rebuild::run(gen_medias(files)),
        Command::List(_) => list::run(gen_medias(files)),
        Command::Rename(_) => rename::run(gen_medias(files)),
        Command::Join(_) => join::run(gen_medias(files)),
    } {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}

fn entries(dir: PathBuf) -> Box<dyn Iterator<Item = PathBuf>> {
    fn is_included(path: &Path) -> Option<bool> {
        fn is_match(s: &str, re_in: Option<&Regex>, re_ex: Option<&Regex>) -> bool {
            re_ex.map_or(true, |re_ex| !re_ex.is_match(s))
                && re_in.map_or(true, |re_in| re_in.is_match(s))
        }

        let (name, ext) = utils::file_stem_ext(path).ok()?; // discards invalid UTF-8 names.
        (!name.starts_with('.')).then_some(())?; // exclude hidden files and folders.

        Some(match path.is_dir() {
            false => {
                // files are checked for name, extension, and parent directory.
                is_match(name, RE_IN.get(), RE_EX.get())
                    && is_match(ext, RE_EXT_IN.get(), RE_EXT_EX.get())
                    && is_match(path.parent()?.to_str()?, RE_DIR_IN.get(), RE_DIR_EX.get())
            }
            true => true, // include all directories that are not hidden, so I can find included subdirectories.
        })
    }

    // now this allows hidden directories, if the user directly asks for them.
    match std::fs::read_dir(&dir) {
        Ok(rd) => Box::new(
            rd.inspect(move |r| {
                if let Err(err) = r {
                    eprintln!("error: read entry {}: {err}", dir.display());
                }
            })
            .flatten()
            .flat_map(move |de| {
                let path = de.path();
                match (is_included(&path).unwrap_or_default(), path.is_dir()) {
                    (true, false) => Box::new(iter::once(path)),
                    (true, true) if !args().shallow && utils::is_running() => entries(path),
                    _ => Box::new(iter::empty()),
                }
            }),
        ),
        Err(err) => {
            eprintln!("error: read dir {}: {err}", dir.display());
            Box::new(iter::empty())
        }
    }
}

fn gen_medias<T>(files: impl Iterator<Item = PathBuf>) -> Vec<T>
where
    T: TryFrom<PathBuf, Error: fmt::Display>,
{
    files
        .map(|p| T::try_from(p))
        .inspect(|m| {
            if let Err(err) = m {
                eprintln!("error: load media: {err}");
            }
        })
        .flatten()
        .collect()
}
