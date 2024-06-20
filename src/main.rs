mod dupes;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::iter;
use std::path::{Path, PathBuf};

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

fn main() -> Result<()> {
    let args = Args::parse();

    // lists files from the given paths, or the current directory if no paths were given.
    let current_dir = args.paths.is_empty().then(|| ".".into());
    let mut files = Box::new(
        args.paths
            .into_iter()
            .chain(current_dir)
            .flat_map(|p| entries(p, args.shallow, args.verbose)),
    ) as Box<dyn Iterator<Item = PathBuf>>;
    if args.verbose {
        files = Box::new(files.inspect(|f| println!("including: {}", f.display())));
    }

    match args.cmd {
        Command::Dupes(dupes) => dupes::find_dupes(files, dupes, args.verbose),
    }
}

fn entries(dir: PathBuf, shallow: bool, verbose: bool) -> Box<dyn Iterator<Item = PathBuf>> {
    fn is_hidden(path: &Path) -> bool {
        path.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with('.'))
    }
    let ignored = move |path: PathBuf| {
        if verbose {
            eprintln!(" ignoring: {}", path.display()); // the size of "including".
        }
        Box::new(iter::empty())
    };

    // now this allows hidden directories, if the user directly asks for them.
    match std::fs::read_dir(&dir) {
        Ok(rd) => Box::new(rd.flat_map(move |r| match r {
            Ok(de) => {
                let path = de.path();
                match (path.is_dir(), is_hidden(&path), shallow) {
                    (false, false, _) => Box::new(iter::once(path)),
                    (true, false, false) => entries(path, shallow, verbose),
                    _ => ignored(path),
                }
            }
            Err(err) => {
                eprintln!("error reading entry {}: {err:?}", dir.display());
                Box::new(iter::empty())
            }
        })),
        Err(err) => {
            eprintln!("error reading dir {}: {err}", dir.display());
            Box::new(iter::empty())
        }
    }
}

fn gen_medias<T>(files: impl Iterator<Item = PathBuf>) -> Vec<T>
where
    T: TryFrom<PathBuf, Error: std::fmt::Debug>,
{
    files
        .flat_map(|p| match T::try_from(p) {
            Ok(m) => Some(m),
            Err(err) => {
                eprintln!("error loading media: {err:?}");
                None
            }
        })
        .collect::<Vec<_>>()
}
