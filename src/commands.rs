mod dupes;
mod join;
mod list;
mod probe;
mod rebuild;
mod rename;

use crate::entries::input::Warnings;
use crate::entries::{Entry, EntrySet, Fetcher};
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
    fn run(self, fetcher: Fetcher, w: Warnings) -> Result<()>;
}

impl<R: Refine> Runner for R {
    fn run(mut self, fetcher: Fetcher, warnings: Warnings) -> Result<()> {
        println!("=> {}\n", R::OPENING_LINE);
        self.tweak(&warnings);
        self.refine(gen_medias(fetcher.fetch(R::HANDLES)))
    }
}

fn view(entries: impl Iterator<Item = Entry>) {
    println!("\nentries seen by this command:\n");
    let mut entries = entries.collect::<Vec<_>>();
    entries.sort_unstable();
    entries.iter().for_each(|e| println!("{e}"));
    println!("\ntotal files: {}", entries.len());
}

impl Command {
    pub fn run(self, fetcher: Fetcher, warnings: Warnings) -> Result<()> {
        match self {
            Command::Dupes(opt) => opt.run(fetcher, warnings),
            Command::Join(opt) => opt.run(fetcher, warnings),
            Command::List(opt) => opt.run(fetcher, warnings),
            Command::Rebuild(opt) => opt.run(fetcher, warnings),
            Command::Rename(opt) => opt.run(fetcher, warnings),
            Command::Probe(opt) => opt.run(fetcher, warnings),
        }
    }

    pub fn view(self, fetcher: Fetcher) {
        let handles = match &self {
            Command::Dupes(_) => dupes::Dupes::HANDLES,
            Command::Join(_) => join::Join::HANDLES,
            Command::List(_) => list::List::HANDLES,
            Command::Rebuild(_) => rebuild::Rebuild::HANDLES,
            Command::Rename(_) => rename::Rename::HANDLES,
            Command::Probe(_) => probe::Probe::HANDLES,
        };
        view(fetcher.fetch(handles));
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
