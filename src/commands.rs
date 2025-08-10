mod dupes;
mod join;
mod list;
mod probe;
mod rebuild;
mod rename;

use crate::entries::{EffectiveInput, Entry, InputInfo, TraversalMode};
use crate::utils::natural_cmp;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Find possibly duplicated files by both size/sample and filename similarity.
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

/// The common interface for commands that refine media files.
pub trait Refine {
    type Media: TryFrom<Entry, Error = (Entry, anyhow::Error)>;

    /// The opening line to display when running the command.
    const OPENING_LINE: &'static str;
    /// The mode of traversal to use when fetching entries.
    const T_MODE: TraversalMode;

    /// Tweak the command options to fix small issues after the opening line, but before fetching
    /// the entries and converting them to the proper Media type.
    fn tweak(&mut self, _: &InputInfo) {}
    /// Actual command implementation, called with the fetched media files.
    fn refine(&self, medias: Vec<Self::Media>) -> Result<()>;
}

// /// The common interface for commands that change the configuration of Refine commands.
// pub trait Configure {
//     /// The opening line to display when running the command.
//     const OPENING_LINE: &'static str;
//
//     /// Actual command implementation.
//     fn config(&self) -> Result<()>;
// }
// fn configure<C: Configure>(mut r: C, fetcher: Fetcher, info: InputSpec) -> Result<()> {
//     println!("=> {}\n", C::OPENING_LINE);
//     r.config()
// }

fn refine<R: Refine>(mut opt: R, ei: EffectiveInput) -> Result<()> {
    println!("=> {}\n", R::OPENING_LINE);
    opt.tweak(&ei.info);
    opt.refine(gen_medias(ei.fetcher.fetch(R::T_MODE)))
}

fn show<R: Refine>(_: R, ei: EffectiveInput) {
    println!("\nentries this command will process:\n");
    let mut entries = ei.fetcher.fetch(R::T_MODE).collect::<Vec<_>>();
    entries.sort_unstable_by(|e, f| natural_cmp(e.to_str(), f.to_str()));
    entries.iter().for_each(|e| println!("{e}"));
    match entries.len() {
        0 => println!("no entries found"),
        n => println!("\ntotal entries: {n}"),
    }
}

impl Command {
    pub fn execute(self, ei: EffectiveInput) -> Result<()> {
        macro_rules! call {
            ($opt:expr) => {
                match ei.show {
                    false => refine($opt, ei),
                    true => Ok(show($opt, ei)),
                }
            };
        }
        match self {
            Command::Dupes(opt) => call!(opt),
            Command::Join(opt) => call!(opt),
            Command::List(opt) => call!(opt),
            Command::Rebuild(opt) => call!(opt),
            Command::Rename(opt) => call!(opt),
            Command::Probe(opt) => call!(opt),
        }
    }
}

fn gen_medias<T>(entries: impl Iterator<Item = Entry>) -> Vec<T>
where
    T: TryFrom<Entry, Error = (Entry, anyhow::Error)>,
{
    entries
        .map(|entry| T::try_from(entry))
        .inspect(|res| {
            if let Err((entry, err)) = res {
                eprintln!("error: load media {entry}: {err}");
            }
        })
        .flatten()
        .collect()
}
