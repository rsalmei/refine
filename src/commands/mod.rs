mod dupes;
mod join;
mod list;
mod rebuild;
mod rename;

use crate::entries::{Expected, Fetcher};
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
/// Implemented for each command's options, conferring its functionality.
pub trait Refine {
    type Media: TryFrom<PathBuf, Error: fmt::Display>;
    const OPENING_LINE: &'static str;
    const EXPECTED: Expected;

    fn adjust(&mut self, _fetcher: &Fetcher) {}
    fn refine(&self, medias: Vec<Self::Media>) -> Result<()>;
}

impl Command {
    pub fn run(self, entries: Fetcher) -> Result<()> {
        match self {
            Command::Dupes(cmd) => run(cmd, entries),
            Command::Rebuild(cmd) => run(cmd, entries),
            Command::List(cmd) => run(cmd, entries),
            Command::Rename(cmd) => run(cmd, entries),
            Command::Join(cmd) => run(cmd, entries),
        }
    }
}

fn run<R: Refine>(mut cmd: R, entries: Fetcher) -> Result<()> {
    println!("=> {}\n", R::OPENING_LINE);
    cmd.adjust(&entries);
    cmd.refine(gen_medias(entries.fetch(R::EXPECTED)))
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
