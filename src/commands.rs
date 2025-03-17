mod dupes;
mod join;
mod list;
mod probe;
mod rebuild;
mod rename;

use crate::Warnings;
use crate::entries::{Entries, Entry, EntrySet};
use anyhow::Result;
use clap::Subcommand;

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
    type Media: TryFrom<Entry, Error = (anyhow::Error, Entry)>;
    const OPENING_LINE: &'static str;
    const HANDLES: EntrySet;

    /// Tweak the command options to fix small issues after the opening line, but before fetching
    /// the entries and converting them to the proper Media type.
    fn tweak(&mut self, _warnings: &Warnings) {}
    /// Actual command implementation, called with the fetched media files.
    fn refine(&self, medias: Vec<Self::Media>) -> Result<()>;
}

trait Runner {
    fn run(self, entries: Entries, w: Warnings) -> Result<()>;
}

impl<R: Refine> Runner for R {
    fn run(mut self, entries: Entries, warnings: Warnings) -> Result<()> {
        println!("=> {}\n", R::OPENING_LINE);
        self.tweak(&warnings);
        self.refine(gen_medias(entries.fetch(R::HANDLES)))
    }
}

impl Command {
    pub fn run(self, entries: Entries, warnings: Warnings) -> Result<()> {
        match self {
            Command::Dupes(options) => options.run(entries, warnings),
            Command::Join(options) => options.run(entries, warnings),
            Command::List(options) => options.run(entries, warnings),
            Command::Rebuild(options) => options.run(entries, warnings),
            Command::Rename(options) => options.run(entries, warnings),
            Command::Probe(options) => options.run(entries, warnings),
        }
    }
}

fn gen_medias<T>(entries: impl Iterator<Item = Entry>) -> Vec<T>
where
    T: TryFrom<Entry, Error = (anyhow::Error, Entry)>,
{
    entries
        .map(|entry| T::try_from(entry))
        .inspect(|res| {
            if let Err((err, entry)) = res {
                eprintln!("error: load media {entry}: {err}");
            }
        })
        .flatten()
        .collect()
}
