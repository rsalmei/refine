use crate::command::Fetcher;
use std::path::PathBuf;

/// The common interface for Refine commands.
///
/// Implemented for each command's options to confer its specific functionality.
pub trait Refine {
    type Media: TryFrom<PathBuf, Error: std::fmt::Display>;
    const OPENING_LINE: &'static str;
    const ENTRY_KIND: EntryKind;

    fn adjust(&mut self, _fetcher: &Fetcher) {}
    fn refine(&self, medias: Vec<Self::Media>) -> anyhow::Result<()>;
}

/// Denotes which kind of entries the command expects.
#[derive(Debug, Copy, Clone)]
pub enum EntryKind {
    /// Only files.
    Files,
    /// Either directories or files, in this order.
    Either,
    /// Both directories and files, in this order.
    Both,
}
