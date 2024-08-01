use clap::Subcommand;

pub mod dupes;
pub mod join;
pub mod list;
pub mod rebuild;
pub mod rename;

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

/// Denotes the kind of entry a command expects.
#[derive(Debug, Copy, Clone)]
pub enum EntryKind {
    /// Only files.
    File,
    /// Either directories or files, in this order.
    Any,
    // Both files and directories.
    // All,
}

#[macro_export]
macro_rules! options {
    ($opt:ident => $conf:expr) => {
        /// The kind of entry this command expects.
        pub const KIND: $crate::commands::EntryKind = $conf;
        /// Retrieves the options given to this command.
        fn opt() -> &'static $opt {
            match &$crate::args().cmd {
                $crate::Command::$opt(opt) => opt,
                _ => unreachable!(),
            }
        }
    };
}
