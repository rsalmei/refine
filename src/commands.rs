mod dupes;
mod join;
mod list;
mod probe;
mod rebuild;
mod rename;

use crate::entries::{Entry, Fetcher, InputInfo, TraversalMode};
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Find possibly duplicated files by both size and filename.
    #[command(override_usage = "refine dupes [DIRS]... [FETCH] [OPTIONS]")]
    Dupes(dupes::Dupes),
    /// Join files into a single directory with advanced conflict resolution.
    #[command(override_usage = "refine join [DIRS]... [FETCH] [OPTIONS]")]
    Join(join::Join),
    /// List files from multiple disjoint directories sorted together.
    #[command(override_usage = "refine list [DIRS]... [FETCH] [OPTIONS]")]
    List(list::List),
    /// Rebuild entire media collections' filenames intelligently.
    #[command(override_usage = "refine rebuild [DIRS]... [FETCH] [OPTIONS]")]
    Rebuild(rebuild::Rebuild),
    /// Rename files and directories in batch using advanced regex rules.
    #[command(override_usage = "refine rename [DIRS]... [FETCH] [OPTIONS]")]
    Rename(rename::Rename),
    /// Probe collections' filenames against a remote server.
    #[command(override_usage = "refine probe [DIRS]... [FETCH] [OPTIONS]")]
    Probe(probe::Probe),
}

/// The common interface for Refine commands that work with media files.
pub trait Refine {
    type Media: for<'a> TryFrom<&'a Entry, Error = anyhow::Error>;
    /// The opening line to display when running the command.
    const OPENING_LINE: &'static str;
    /// The mode of traversal to use when fetching entries.
    const MODE: TraversalMode;

    /// Tweak the command options to fix small issues after the opening line, but before fetching
    /// the entries and converting them to the proper Media type.
    fn tweak(&mut self, _: &InputInfo) {}
    /// Actual command implementation, called with the fetched media files.
    fn refine(&self, medias: Vec<Self::Media>) -> Result<()>;
}

trait Runner {
    fn run(self, fetcher: Fetcher, w: InputInfo) -> Result<()>;
}

impl<R: Refine> Runner for R {
    fn run(mut self, fetcher: Fetcher, info: InputInfo) -> Result<()> {
        println!("=> {}\n", R::OPENING_LINE);
        self.tweak(&info);
        self.refine(gen_medias(fetcher.fetch(R::MODE)))
    }
}

impl Command {
    pub fn run(self, fetcher: Fetcher, info: InputInfo) -> Result<()> {
        match self {
            Command::Dupes(opt) => opt.run(fetcher, info),
            Command::Join(opt) => opt.run(fetcher, info),
            Command::List(opt) => opt.run(fetcher, info),
            Command::Rebuild(opt) => opt.run(fetcher, info),
            Command::Rename(opt) => opt.run(fetcher, info),
            Command::Probe(opt) => opt.run(fetcher, info),
        }
    }

    pub fn traversal_mode(&self) -> TraversalMode {
        match &self {
            Command::Dupes(_) => dupes::Dupes::MODE,
            Command::Join(_) => join::Join::MODE,
            Command::List(_) => list::List::MODE,
            Command::Rebuild(_) => rebuild::Rebuild::MODE,
            Command::Rename(_) => rename::Rename::MODE,
            Command::Probe(_) => probe::Probe::MODE,
        }
    }
}

fn gen_medias<T>(entries: impl Iterator<Item = Entry>) -> Vec<T>
where
    T: for<'a> TryFrom<&'a Entry, Error = anyhow::Error>,
{
    entries
        .map(|entry| T::try_from(&entry).map_err(|err| (err, entry)))
        .inspect(|res| {
            if let Err((err, entry)) = res {
                eprintln!("error: load media {entry}: {err}");
            }
        })
        .flatten()
        .collect()
}
