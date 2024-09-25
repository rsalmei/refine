mod dupes;
mod join;
mod list;
mod rebuild;
mod rename;

use crate::entries::{find_entries, EntryKind, Filters};
use clap::Subcommand;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Command {
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

pub trait Refine {
    type Media: TryFrom<PathBuf, Error: fmt::Display>;
    const OPENING_LINE: &'static str;
    const ENTRY_KIND: EntryKind;

    fn refine(self, medias: Vec<Self::Media>) -> anyhow::Result<()>;
}

pub fn run<R: Refine>(cmd: R, (paths, filters): (Vec<PathBuf>, Filters)) -> anyhow::Result<()> {
    println!("=> {}\n", R::OPENING_LINE);
    cmd.refine(gen_medias(find_entries(filters, paths, R::ENTRY_KIND)?))
}

fn gen_medias<T>(entries: impl Iterator<Item = PathBuf>) -> Vec<T>
where
    T: TryFrom<PathBuf, Error: fmt::Display>,
{
    entries
        .map(|path| T::try_from(path))
        .inspect(|res| {
            if let Err(err) = res {
                eprintln!("error: load media: {err}");
            }
        })
        .flatten()
        .collect()
}

/// Optional static variable that holds the options given to a command.
///
/// Useful for sharing the options out of the main refine function.
#[macro_export]
macro_rules! options {
    ($opt:ty) => {
        static OPTIONS: std::sync::OnceLock<$opt> = std::sync::OnceLock::new();
        /// Retrieves the options given to this command.
        fn opt() -> &'static $opt {
            OPTIONS.get().unwrap()
        }
    };
    (=> $opt:expr) => {
        OPTIONS.set($opt).unwrap();
    };
}
