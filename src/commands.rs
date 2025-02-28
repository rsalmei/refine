mod dupes;
mod join;
mod list;
mod probe;
mod rebuild;
mod rename;

use crate::entries::{Entries, Entry, EntryKinds, Warnings};
use anyhow::Result;
use clap::Subcommand;
use std::fmt;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Find reasonably duplicated files by both size and filename.
    Dupes(dupes::Dupes),
    /// Join files into a single directory with advanced conflict resolution.
    Join(join::Join),
    /// List files from multiple directories sorted together.
    List(list::List),
    /// Rebuild entire media collections intelligently.
    Rebuild(rebuild::Rebuild),
    /// Rename files and directories using advanced regular expression rules.
    Rename(rename::Rename),
    /// Probe filenames against a remote server.
    Probe(probe::Probe),
}

/// The common interface for Refine commands that work with media files.
pub trait Refine {
    type Media: TryFrom<Entry, Error: fmt::Display>;
    const OPENING_LINE: &'static str;
    const REQUIRE: EntryKinds;

    fn prepare(&mut self, _: &Warnings) -> Result<()> {
        Ok(())
    }
    fn refine(&self, medias: Vec<Self::Media>) -> Result<()>;
}

trait Runner {
    fn run(self, entries: Entries) -> Result<()>;
}

impl<R: Refine> Runner for R {
    fn run(mut self, entries: Entries) -> Result<()> {
        self.prepare(entries.warnings())?;
        println!("=> {}\n", R::OPENING_LINE);
        self.refine(gen_medias(entries.fetch(R::REQUIRE)))
    }
}

impl Command {
    pub fn run(self, entries: Entries) -> Result<()> {
        match self {
            Command::Dupes(options) => options.run(entries),
            Command::Join(options) => options.run(entries),
            Command::List(options) => options.run(entries),
            Command::Rebuild(options) => options.run(entries),
            Command::Rename(options) => options.run(entries),
            Command::Probe(options) => options.run(entries),
        }
    }
}

fn gen_medias<T>(paths: impl Iterator<Item = Entry>) -> Vec<T>
where
    T: TryFrom<Entry, Error: fmt::Display>,
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
