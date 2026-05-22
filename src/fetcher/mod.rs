mod build;
mod filter;
mod recursion;

use crate::entries::Entry;
use crate::{error, utils};
use clap::Args;
use std::iter;
use std::path::PathBuf;
use std::rc::Rc;

pub use build::*;
pub use filter::*;
pub use recursion::*;

#[derive(Debug, Args)]
pub struct FetcherArgs {
    /// The directories to scan.
    // #[arg(help_heading = None, conflicts_with = "collection")]
    #[arg(help_heading = None)]
    dirs: Vec<PathBuf>,
    /// The collection name to use, which supplies the paths to scan.
    // #[arg(long, value_name = "STR", help_heading = None, conflicts_with = "dirs")]
    // collection: Option<String>,
    #[command(flatten)]
    recursion: RecursionArgs,
    #[command(flatten)]
    filter: FilterArgs,
}

/// The object that fetches and filters entries from multiple directories.
#[derive(Debug)]
pub struct Fetcher {
    dirs: Vec<Entry>,
    recursion: Recursion,
    filter: Filter,
}

/// The policy for how directories are handled when fetching entries.
///
/// All modes always yield matching files. The real differentiator is what happens with directories:
/// whether they're never yielded, yielded immediately, yielded with their contents, or yielded only
/// at the depth limit.
#[derive(Debug, Copy, Clone)]
pub enum DirPolicy {
    /// Never yield dirs, only traverse through them (dupes, probe, rebuild).
    Never,
    /// Yield dirs immediately when found, without entering them (join).
    Immediately,
    /// Yield dirs and also enter them for their contents (rename).
    WithContents,
    /// Yield dirs only when the depth limit is hit (list).
    AtLimit,
}

impl Fetcher {
    /// Create a fetcher for a single entry, with the given recursion mode and default filter.
    pub fn single(entry: &Entry, recursion: Recursion) -> Self {
        Fetcher {
            dirs: vec![entry.to_owned()],
            recursion,
            filter: Filter::default(),
        }
    }

    /// Start fetching entries according to the fetcher's configuration and the given directory
    /// policy.
    pub fn fetch(self, dp: DirPolicy) -> impl Iterator<Item = Entry> {
        let filter = Rc::new(self.filter);
        self.dirs
            .into_iter()
            .flat_map(move |dir| entries(dir, self.recursion, dp, Rc::clone(&filter)))
    }

    /// Return the length of the common prefix of the string representations of input entries.
    ///
    /// This is used to reduce the amount of redundant path information printed in the screen, by only
    /// printing the unique suffix of each entry after the common prefix. Also, it finds the last
    /// directory separator in the common prefix, so that the unique suffixes are always whole dirs.
    /// For example, if the inputs were "/music/rock" and "/music/rock-classics", the common prefix
    /// would be "/music/" and the return value would be 7.
    pub fn find_common_dir_prefix_length(&self) -> usize {
        let mut iter = self.dirs.iter().map(|e| e.to_str());
        let Some(first) = iter.next() else {
            return 0;
        };
        // fold all entries, shrinking the common prefix on each step.
        // bytes are safe here: all paths are valid UTF-8, and '/' is single-byte ASCII.
        let common = iter.fold(first, |acc, s| {
            let common_len = acc
                .bytes()
                .zip(s.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            &acc[..common_len]
        });
        // find the last directory separator in the common prefix.
        common.rfind('/').unwrap_or_default()
    }
}

/// Fetch entries from a single directory, according to the given parameters. This is the core
/// recursive function that implements the fetching logic, and is called for each directory to fetch
/// its entries.
fn entries(
    dir: Entry,
    recursion: Recursion,
    dp: DirPolicy,
    f: Rc<Filter>,
) -> Box<dyn Iterator<Item = Entry>> {
    if !utils::is_running() {
        return Box::new(iter::empty());
    }

    // this does allow hidden directories, if the user directly asks for them.
    match std::fs::read_dir(&dir) {
        Ok(rd) => Box::new(
            rd.inspect(|res| {
                if let Err(err) = res {
                    error!("dir entry: {err}");
                }
            })
            .flatten()
            .map(move |de| {
                de.file_name()
                    .to_str()
                    .map(|s| dir.join(s))
                    .ok_or_else(|| Box::new(de))
            })
            .inspect(|res| {
                if let Err(de) = res {
                    error!("no UTF-8 name: {de:?}");
                }
            })
            .flatten()
            .flat_map(move |entry| {
                use DirPolicy::*;
                if !entry.is_dir() {
                    // files that pass the filter are always included in any mode.
                    return if f.is_in(&entry) && !entry.file_name().starts_with(".") {
                        Box::new(iter::once(entry)) as Box<dyn Iterator<Item = _>>
                    } else {
                        Box::new(iter::empty())
                    };
                }
                // if the entry is a directory, it's much more complicated.
                match (f.is_in(&entry), (dp, recursion.deeper())) {
                    // cases that the directory is yielded and not recursed into.
                    (true, (WithContents | AtLimit, None) | (Immediately, _)) => {
                        Box::new(iter::once(entry))
                    }
                    // the directory is yielded with its content and recursed into.
                    (true, (WithContents, Some(r))) => Box::new(
                        iter::once(entry.clone()).chain(entries(entry, r, dp, Rc::clone(&f))),
                    ),
                    // recurse into dirs if depth available, to find more matching entries deeper in the hierarchy.
                    (_, (_, Some(r))) if !entry.file_name().starts_with(".") => {
                        entries(entry, r, dp, Rc::clone(&f))
                    }
                    _ => Box::new(iter::empty()),
                }
            }),
        ),
        Err(err) => {
            error!("read dir {dir}: {err}");
            Box::new(iter::empty())
        }
    }
}
