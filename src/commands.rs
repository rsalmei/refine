mod dupes;
mod join;
mod list;
mod rebuild;
mod rename;

use crate::entries;
use clap::Subcommand;
use entries::EntryKind;
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

    fn entry_kind() -> EntryKind;
    fn refine(self, medias: Vec<Self::Media>) -> anyhow::Result<()>;
}

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
