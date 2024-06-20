mod dupes;

use anyhow::Result;
use clap::{Parser, Subcommand};
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
    /// Verbose output.
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Find possibly duplicated files by both size and name.
    Dupes(dupes::Dupes),
}

static ARGS: OnceLock<Args> = OnceLock::new();
fn args() -> &'static Args {
    ARGS.get().unwrap()
}

fn main() -> Result<()> {
    ARGS.set(Args::parse()).unwrap();

    // lists files from the given paths, or the current directory if no paths were given.
    let cd = args().paths.is_empty().then(|| ".".into());
    let mut files = Box::new(args().paths.iter().cloned().chain(cd).flat_map(entries))
        as Box<dyn Iterator<Item = PathBuf>>;
    if args().verbose {
        files = Box::new(files.inspect(|f| println!("including: {}", f.display())));
    }

    match args().cmd {
        Command::Dupes(_) => dupes::find_dupes(gen_medias(files)),
    }
}

fn entries(dir: PathBuf) -> Box<dyn Iterator<Item = PathBuf>> {
    fn is_hidden(path: &Path) -> bool {
        path.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with('.'))
    }
    fn ignored(path: PathBuf) -> Box<dyn Iterator<Item = PathBuf>> {
        if args().verbose {
            eprintln!(" ignoring: {}", path.display()); // the size of "including".
        }
        Box::new(iter::empty())
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
                match (path.is_dir(), is_hidden(&path), args().shallow) {
                    (false, false, _) => Box::new(iter::once(path)),
                    (true, false, false) => entries(path),
                    _ => ignored(path),
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
