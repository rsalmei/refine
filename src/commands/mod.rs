mod dupes;
mod join;
mod list;
mod rebuild;
mod rename;

use crate::entries::{EntryKind, Fetcher, Filters};
use anyhow::Result;
use clap::Subcommand;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Find possibly duplicated files by both size and filename.
    Dupes(dupes::Dupes),
    /// Join all files into the same directory.
    Join(join::Join),
    /// List files from the given paths.
    List(list::List),
    /// Rebuild the filenames of media collections intelligently.
    Rebuild(rebuild::Rebuild),
    /// Rename files in batch, according to the given rules.
    Rename(rename::Rename),
}

/// The common interface for Refine commands.
///
/// Implemented for each command's options to confer its specific functionality.
pub trait Refine {
    type Media: TryFrom<PathBuf, Error: fmt::Display>;
    const OPENING_LINE: &'static str;
    const ENTRY_KIND: EntryKind;

    fn adjust(&mut self, _fetcher: &Fetcher) {}
    fn refine(&self, medias: Vec<Self::Media>) -> Result<()>;
}

impl Command {
    pub fn run(self, dirs: Vec<PathBuf>, filters: Filters) -> Result<()> {
        let fetcher = Fetcher::new(dirs, filters)?;
        match self {
            Command::Dupes(cmd) => run(cmd, fetcher),
            Command::Rebuild(cmd) => run(cmd, fetcher),
            Command::List(cmd) => run(cmd, fetcher),
            Command::Rename(cmd) => run(cmd, fetcher),
            Command::Join(cmd) => run(cmd, fetcher),
        }
    }
}

fn run<R: Refine>(mut cmd: R, fetcher: Fetcher) -> Result<()> {
    println!("=> {}\n", R::OPENING_LINE);
    cmd.adjust(&fetcher);
    cmd.refine(gen_medias(fetcher.fetch(R::ENTRY_KIND)))
}

fn gen_medias<T>(paths: impl Iterator<Item = PathBuf>) -> Vec<T>
where
    T: TryFrom<PathBuf, Error: fmt::Display>,
{
    paths
        .map(|path| T::try_from(path))
        .inspect(|res| {
            if let Err(err) = res {
                eprintln!("error: load media: {err}");
            }
        })
        .flatten()
        .collect()
}
