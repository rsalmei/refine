mod dupes;
mod rebuild;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{fmt, iter};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
    /// Paths to scan.
    #[arg(global = true)]
    paths: Vec<PathBuf>,
    /// Include only some of the accessible files; tested against the whole filename, including extension.
    #[arg(short, long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    include: Option<String>,
    /// Do not recurse into subdirectories.
    #[arg(long, global = true)]
    shallow: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Find possibly duplicated files by both size and name.
    Dupes(dupes::Dupes),
    /// Rebuild names of collections of files intelligently.
    Rebuild(rebuild::Rebuild),
}

static ARGS: OnceLock<Args> = OnceLock::new();
static RE_IN: OnceLock<Regex> = OnceLock::new();
fn args() -> &'static Args {
    ARGS.get().unwrap()
}

fn main() -> Result<()> {
    ARGS.set(Args::parse()).unwrap();
    println!("Refine: v{}", env!("CARGO_PKG_VERSION"));

    if let Some(s) = &args().include {
        match Regex::new(s) {
            Ok(re) => RE_IN.set(re).unwrap(),
            Err(err) => {
                eprintln!("error: invalid --include regex: {err:?}");
                std::process::exit(1);
            }
        }
    }

    // lists files from the given paths, or the current directory if no paths were given.
    let cd = args().paths.is_empty().then(|| ".".into());
    let files = Box::new(args().paths.iter().cloned().chain(cd).flat_map(entries))
        as Box<dyn Iterator<Item = PathBuf>>;

    match args().cmd {
        Command::Dupes(_) => dupes::find_dupes(gen_medias(files)),
        Command::Rebuild(_) => rebuild::rebuild(gen_medias(files)),
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
                    eprintln!("error reading entry {}: {err:?}", dir.display());
                }
            })
            .flatten()
            .flat_map(move |de| {
                let path = de.path();
                match (is_included(&path).unwrap_or_default(), path.is_dir()) {
                    (true, false) => Box::new(iter::once(path)),
                    (true, true) => entries(path),
                    _ => Box::new(iter::empty()),
                }
            }),
        ),
        Err(err) => {
            eprintln!("error reading dir {}: {err}", dir.display());
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
                eprintln!("error loading media: {err:?}");
            }
        })
        .flatten()
        .collect::<Vec<_>>()
}
