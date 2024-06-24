mod dupes;
mod rebuild;

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
fn args() -> &'static Args {
    ARGS.get().unwrap()
}

fn main() -> Result<()> {
    ARGS.set(Args::parse()).unwrap();
    println!("Refine: v{}", env!("CARGO_PKG_VERSION"));

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
    fn is_hidden(path: &Path) -> bool {
        path.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with('.'))
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
                match (path.is_dir(), is_hidden(&path)) {
                    (false, false) => Box::new(iter::once(path)),
                    (true, false) if !args().shallow => entries(path),
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

/// Util function to strip sequence numbers from a filename.
fn strip_sequence(name: &str) -> &str {
    static RE_MULTI_MACOS: OnceLock<Regex> = OnceLock::new();
    static RE_MULTI_LOCAL: OnceLock<Regex> = OnceLock::new();
    let rem = RE_MULTI_MACOS.get_or_init(|| Regex::new(r" copy( \d+)?$").unwrap());
    let rel = RE_MULTI_LOCAL.get_or_init(|| Regex::new(r"-\d+$").unwrap());

    let name = rem.split(name).next().unwrap(); // even if the name is " copy", this returns an empty str.
    rel.split(name).next().unwrap() // same as above, even if the name is "-1", this returns an empty str.
}
