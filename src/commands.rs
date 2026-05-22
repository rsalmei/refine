mod dupes;
mod join;
mod list;
mod probe;
mod rebuild;
mod rename;

use crate::entries::Entry;
use crate::fetcher::{DirPolicy, FetcherArgs, InputInfo};
use crate::{error, utils};
use anyhow::Result;
use clap::{Args, Subcommand};

/// The common arguments for all refine commands, which handle media files. They include the
/// specific command options, the fetcher arguments, and some modifiers.
#[derive(Debug, Args)]
pub struct CommandArgs<T: Args> {
    #[command(flatten)]
    cmd: T,
    #[command(flatten)]
    args: FetcherArgs,
    #[command(flatten)]
    mods: Modifiers,
}

#[derive(Debug, Args)]
#[command(next_help_heading = Some("Modifiers"))]
pub struct Modifiers {
    /// Show the entries that will be processed, and ask for confirmation before proceeding.
    #[arg(long)]
    pub show: bool,
    /// Do not strip the common prefix from the input paths when displaying them.
    #[arg(long)]
    pub full: bool,
}

/// Subcommands for refining media files.
///
/// They all share some [input] arguments and modifiers, and include their own specific options.
#[derive(Debug, Subcommand)]
pub enum RefineCommand {
    /// Find possibly duplicated files by both size/sample and filename similarity.
    #[command(override_usage = "refine dupes [DIRS]... [FETCH] [OPTIONS]")]
    Dupes(CommandArgs<dupes::Dupes>),
    /// Join files into a single directory with advanced conflict resolution.
    #[command(override_usage = "refine join [DIRS]... [FETCH] [OPTIONS]")]
    Join(CommandArgs<join::Join>),
    /// List files from multiple disjoint directories sorted together.
    #[command(override_usage = "refine list [DIRS]... [FETCH] [OPTIONS]")]
    List(CommandArgs<list::List>),
    /// Rebuild entire media collections' filenames intelligently.
    #[command(override_usage = "refine rebuild [DIRS]... [FETCH] [OPTIONS]")]
    Rebuild(CommandArgs<rebuild::Rebuild>),
    /// Rename files and directories in batch using advanced regex rules.
    #[command(override_usage = "refine rename [DIRS]... [FETCH] [OPTIONS]")]
    Rename(CommandArgs<rename::Rename>),
    /// Probe collections' filenames against a remote server.
    #[command(override_usage = "refine probe [DIRS]... [FETCH] [OPTIONS]")]
    Probe(CommandArgs<probe::Probe>),
}

/// The common interface for commands that refine media files.
pub trait Refine {
    type Media: TryFrom<Entry, Error = (Entry, anyhow::Error)>;

    /// The opening line to display when running the command.
    const OPENING_LINE: &'static str;
    /// The mode of traversal to use when fetching entries.
    const DIR_POLICY: DirPolicy;

    /// Tweak the command options before fetching, for example to automatically enable some options
    /// if the input has multiple directories or invalid ones.
    fn tweak(&mut self, _: &InputInfo) {}

    /// Peek at the fetched media files to also tweak the command options, for example to
    /// automatically enable some options if the media files are scattered across multiple
    /// subdirectories or contained in a single one.
    fn peek(&mut self, _: &[Self::Media]) {}

    /// Actual command implementation, called with the fetched media files.
    fn refine(&self, medias: Vec<Self::Media>) -> Result<()>;
}

impl RefineCommand {
    pub fn run(self) -> Result<()> {
        match self {
            RefineCommand::Dupes(cmd_args) => run(cmd_args),
            RefineCommand::Join(cmd_args) => run(cmd_args),
            RefineCommand::List(cmd_args) => run(cmd_args),
            RefineCommand::Rebuild(cmd_args) => run(cmd_args),
            RefineCommand::Rename(cmd_args) => run(cmd_args),
            RefineCommand::Probe(cmd_args) => run(cmd_args),
        }
    }
}

fn run<R: Refine + Args>(ca: CommandArgs<R>) -> Result<()> {
    println!("{}\n", R::OPENING_LINE);

    let (fetcher, info) = ca.args.try_into()?; // validate input and prepare fetcher, AFTER the opening line so that errors are displayed after it.
    if !ca.mods.full {
        Entry::prefix_len(fetcher.find_common_dir_prefix_length()); // set the common prefix length for display purposes.
    }

    let iter: Box<dyn Iterator<Item = Entry>> = if ca.mods.show {
        println!("Entries this command will process:");
        // allocate all entries up front to sort and display them.
        let mut entries = fetcher.fetch(R::DIR_POLICY).collect::<Vec<_>>();
        entries.sort_unstable_by(|e, f| utils::natural_cmp(e.to_str(), f.to_str()));
        entries.iter().for_each(|e| println!("{e}"));
        if entries.is_empty() {
            println!("no entries found");
            return Ok(());
        }

        println!("\ntotal entries: {}", entries.len());
        utils::prompt_yes_no("Do you want to continue?")?;
        println!();
        Box::new(entries.into_iter())
    } else {
        // keep reading entries lazily, allocating only once.
        Box::new(fetcher.fetch(R::DIR_POLICY))
    };

    let mut cmd = ca.cmd;
    cmd.tweak(&info);
    let medias = gen_medias(iter);
    cmd.peek(&medias);
    cmd.refine(medias)
}

fn gen_medias<T>(entries: impl Iterator<Item = Entry>) -> Vec<T>
where
    T: TryFrom<Entry, Error = (Entry, anyhow::Error)>,
{
    entries
        .map(|entry| T::try_from(entry))
        .inspect(|res| {
            if let Err((entry, err)) = res {
                error!("load media {entry}: {err}");
            }
        })
        .flatten()
        .collect()
}
