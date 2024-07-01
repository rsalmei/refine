mod dupes;
mod list;
mod rebuild;
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
    /// Include only some files; tested against filename+extension, case-insensitive.
    #[arg(short, long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    include: Option<String>,
    /// Do not recurse into subdirectories.
    #[arg(long, global = true, help_heading = Some("Global"))]
    shallow: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Find possibly duplicated files by both size and filename.
    Dupes(dupes::Dupes),
    /// Rebuild the filenames of collections of files intelligently.
    Rebuild(rebuild::Rebuild),
    /// List files from the given paths.
    List(list::List),
}

static RE_IN: OnceLock<Regex> = OnceLock::new();

static ARGS: OnceLock<Args> = OnceLock::new();
fn args() -> &'static Args {
    ARGS.get().unwrap()
}

fn main() {
    ARGS.set(Args::parse()).unwrap();
    println!("Refine: v{}", env!("CARGO_PKG_VERSION"));

    ctrlc::set_handler({
        let running = Arc::clone(utils::running_flag());
        move || {
            eprintln!("aborting...");
            running.store(false, atomic::Ordering::Relaxed);
        }
    })
    .expect("Error setting Ctrl-C handler");

    if let Some(s) = &args().include {
        match Regex::new(&format!("(?i){s}")) {
            Ok(re) => RE_IN.set(re).unwrap(),
            Err(err) => {
                eprintln!("error: invalid --include regex: {err:?}");
                std::process::exit(1);
            }
        }
    }

    // lists files from the given paths, or the current directory if no paths were given.
    let cd = args().paths.is_empty().then(|| ".".into());
    let files = args().paths.iter().cloned().chain(cd).flat_map(entries);

    if let Err(err) = match args().cmd {
        Command::Dupes(_) => dupes::find_dupes(gen_medias(files)),
        Command::Rebuild(_) => rebuild::rebuild(gen_medias(files)),
        Command::List(_) => list::list(gen_medias(files)),
    } {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}

fn entries(dir: PathBuf) -> Box<dyn Iterator<Item = PathBuf>> {
    fn is_included(path: &Path) -> Option<bool> {
        let name = path.file_name()?.to_str()?;
        (!name.starts_with('.')).then_some(())?; // exclude hidden files and folders.
        match (path.is_dir(), &args().include) {
            (true, _) if !args().shallow => Some(true),
            (false, None) => Some(true),
            (false, Some(_)) => Some(RE_IN.get().unwrap().is_match(name)),
            _ => None,
        }
    }

    // now this allows hidden directories, if the user directly asks for them.
    match std::fs::read_dir(&dir) {
        Ok(rd) => Box::new(
            rd.inspect(move |r| {
                if let Err(err) = r {
                    eprintln!("error: read entry {}: {err:?}", dir.display());
                }
            })
            .flatten()
            .flat_map(move |de| {
                let path = de.path();
                match (is_included(&path).unwrap_or_default(), path.is_dir()) {
                    (true, false) => Box::new(iter::once(path)),
                    (true, true) if utils::running() => entries(path),
                    _ => Box::new(iter::empty()),
                }
            }),
        ),
        Err(err) => {
            eprintln!("error: read dir {}: {err:?}", dir.display());
            Box::new(iter::empty())
        }
    }
}

fn gen_medias<T>(files: impl Iterator<Item = PathBuf>) -> Vec<T>
where
    T: TryFrom<PathBuf, Error: fmt::Debug>,
{
    files
        .map(|p| T::try_from(p))
        .inspect(|m| {
            if let Err(err) = m {
                eprintln!("error: load media: {err:?}");
            }
        })
        .flatten()
        .collect()
}
