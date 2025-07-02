mod entry;
mod filter;
mod input;

use crate::utils;
use anyhow::{Result, anyhow};
pub use entry::*;
pub use filter::*;
pub use input::*;
use std::iter;
use std::rc::Rc;

/// The object that fetches and filters entries from multiple directories.
#[derive(Debug)]
pub struct Fetcher {
    /// Effective input paths to scan, after deduplication and checking.
    dirs: Vec<Entry>,
    recurse: Recurse,
    filter_rules: FilterRules,
}

/// The mode of traversal to use when fetching entries.
#[derive(Debug, Copy, Clone)]
pub enum TraversalMode {
    /// Only files (dupes, probe, and rebuild).
    Files,
    /// Directories stop recursion because the dir itself is the output (join).
    DirsStop,
    /// Directories are chained with their content (rename).
    DirsAndContent,
    /// Contents are listed while recursing, and change to directories at the max depth (list).
    ContentOverDirs,
}

/// The friendly recursion mode for fetching entries.
#[derive(Debug)]
pub enum Recurse {
    Full,
    Shallow,
    UpTo(u32),
}

impl Fetcher {
    /// Reads all entries from a single directory.
    pub fn single(entry: impl Into<Entry>, recurse: Recurse) -> Self {
        Self::new(vec![entry.into()], recurse, FilterSpec::default()).unwrap() // can't fail.
    }

    /// Reads entries from the given directories, with the given filtering rules and recursion.
    pub fn new(dirs: Vec<Entry>, recurse: Recurse, filter: FilterSpec) -> Result<Self> {
        let filter_rules = filter.try_into()?; // compile regexes and check for errors before anything else.
        if dirs.is_empty() {
            return Err(anyhow!("no valid paths given"));
        }
        Ok(Fetcher {
            dirs,
            recurse,
            filter_rules,
        })
    }

    pub fn fetch(self, mode: TraversalMode) -> impl Iterator<Item = Entry> {
        let depth = self.recurse.into();
        let fr = Rc::new(self.filter_rules);
        self.dirs
            .into_iter()
            .flat_map(move |dir| entries(dir, depth, mode, Rc::clone(&fr)))
    }
}

fn entries(
    dir: Entry,
    depth: Depth,
    mode: TraversalMode,
    fr: Rc<FilterRules>,
) -> Box<dyn Iterator<Item = Entry>> {
    if !utils::is_running() {
        return Box::new(iter::empty());
    }

    // this does allow hidden directories, if the user directly asks for them.
    match std::fs::read_dir(&dir) {
        Ok(rd) => Box::new(
            rd.inspect(|res| {
                if let Err(err) = res {
                    eprintln!("error: dir entry: {err}");
                }
            })
            .flatten()
            .map(move |de| de.file_name().to_str().map(|s| dir.join(s)).ok_or(de))
            .inspect(|res| {
                if let Err(de) = res {
                    eprintln!("error: no UTF-8 name: {de:?}");
                }
            })
            .flatten()
            .flat_map(move |entry| {
                use TraversalMode::*;
                if !entry.is_dir() {
                    // files that pass the filter are always included in any mode.
                    return if fr.is_in(&entry) && !entry.starts_with(".") {
                        Box::new(iter::once(entry)) as Box<dyn Iterator<Item = _>>
                    } else {
                        Box::new(iter::empty())
                    };
                }
                // if the entry is a directory, it's much more complicated.
                match (fr.is_in(&entry), (mode, depth.deeper())) {
                    // cases that the directory is yielded and not recursed into.
                    (true, (DirsAndContent | ContentOverDirs, None) | (DirsStop, _)) => {
                        Box::new(iter::once(entry))
                    }
                    // the directory is yielded with its content and recursed into.
                    (true, (DirsAndContent, Some(d))) => Box::new(
                        iter::once(entry.clone()).chain(entries(entry, d, mode, Rc::clone(&fr))),
                    ),
                    // recurse into dirs if depth available, to find more matching entries deeper in the hierarchy.
                    (_, (_, Some(d))) if !entry.starts_with(".") => {
                        entries(entry, d, mode, Rc::clone(&fr))
                    }
                    _ => Box::new(iter::empty()),
                }
            }),
        ),
        Err(err) => {
            eprintln!("error: read dir {dir}: {err}");
            Box::new(iter::empty())
        }
    }
}

impl From<u32> for Recurse {
    fn from(d: u32) -> Self {
        match d {
            0 => Recurse::Full,
            1 => Recurse::Shallow,
            _ => Recurse::UpTo(d),
        }
    }
}

impl From<Recurse> for Depth {
    fn from(r: Recurse) -> Self {
        match r {
            Recurse::Full => Depth { max: 0, curr: 0 },
            Recurse::Shallow => Depth { max: 1, curr: 0 },
            Recurse::UpTo(d) => Depth { max: d, curr: 0 },
        }
    }
}

/// Used to track the depth of recursion when fetching entries.
#[derive(Debug, Copy, Clone)]
struct Depth {
    curr: u32,
    max: u32,
}

impl Depth {
    fn deeper(self) -> Option<Self> {
        let Depth { curr, max } = self;
        let curr = curr + 1;
        (curr < max || max == 0).then_some(Depth { curr, max })
    }
}
