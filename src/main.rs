mod dupes;

use clap::{Parser, Subcommand};
use std::iter;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
    /// Paths to scan.
    #[arg(global = true)]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Find possibly duplicated files in paths based on both size and name.
    Dupes(dupes::Dupes),
}

fn main() {
    let args = Args::parse();

    // Lists files from the given paths, or the current directory if no paths are given.
    let current_dir = args.paths.is_empty().then(|| ".".into());
    let files = args.paths.into_iter().chain(current_dir).flat_map(entries);

    let res = match args.cmd {
        Command::Dupes(dupes) => dupes::find_dupes(files, dupes),
    };
    if let Err(err) = res {
        eprintln!("{err:?}");
    }
}

fn entries(path: PathBuf) -> Box<dyn Iterator<Item = PathBuf>> {
    if path // ignores both hidden dirs and dotfiles.
        .file_name()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.starts_with('.'))
    {
        return Box::new(iter::empty());
    }

    match path.is_dir() {
        false if path.exists() => Box::new(iter::once(path)),
        false => {
            eprintln!("unknown path: {}", path.display());
            Box::new(iter::empty())
        }
        true => match std::fs::read_dir(&path) {
            Ok(rd) => Box::new(rd.flat_map(move |r| match r {
                Ok(dir) => entries(dir.path()),
                Err(err) => {
                    eprintln!("{} -> {err:?}", path.display());
                    Box::new(iter::empty())
                }
            })),
            Err(err) => {
                eprintln!("{} -> {err:?}", path.display());
                Box::new(iter::empty())
            }
        },
    }
}
