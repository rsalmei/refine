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
    filter: Rc<EntryFilter>,
}

/// The mode of traversal to use when fetching entries.
#[derive(Debug, Copy, Clone)]
pub enum TraversalMode {
    /// Only files.
    Files, // dupes, probe, and rebuild.
    /// Directories stop recursion because the dir itself is the output.
    DirsStop, // join.
    /// Directories are chained with their content.
    DirsAndContent, // rename.
    /// Contents are listed while recursing, and change to directories at the max depth.
    ContentOverDirs, // list
}

#[derive(Debug)]
pub enum Recurse {
    Full,
    Shallow,
    UpTo(u32),
}

impl Fetcher {
    /// Reads all entries from a single directory.
    pub fn single(entry: impl Into<Entry>, recurse: Recurse) -> Self {
        Self::new(vec![entry.into()], recurse, Filter::default()).unwrap() // can't fail.
    }

    /// Reads entries from the given directories, with the given filtering rules and recursion.
    pub fn new(dirs: Vec<Entry>, recurse: Recurse, filter: Filter) -> Result<Self> {
        let filter = filter.try_into()?; // compile regexes and check for errors before anything else.

        if dirs.is_empty() {
            return Err(anyhow!("no valid paths given"));
        }

        Ok(Fetcher {
            dirs,
            recurse,
            filter: Rc::new(filter),
        })
    }

    pub fn fetch(self, mode: TraversalMode) -> impl Iterator<Item = Entry> {
        let depth = self.recurse.into();
        self.dirs
            .into_iter()
            .flat_map(move |dir| entries(dir, depth, mode, Rc::clone(&self.filter)))
    }
}

fn entries(
    dir: Entry,
    depth: Depth,
    mode: TraversalMode,
    ef: Rc<EntryFilter>,
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
                let (d, rec) = depth.inc();
                match (entry.is_dir(), ef.is_in(&entry), rec, mode) {
                    (false, true, _, _) => Box::new(iter::once(entry)),
                    (true, true, false, DirsStop | DirsAndContent | ContentOverDirs) => {
                        Box::new(iter::once(entry))
                    }
                    (true, true, true, Files | ContentOverDirs) => {
                        entries(entry, d, mode, Rc::clone(&ef))
                    }
                    (true, true, true, DirsStop) => Box::new(iter::once(entry)),
                    (true, true, true, DirsAndContent) => Box::new(
                        iter::once(entry.clone()).chain(entries(entry, d, mode, Rc::clone(&ef))),
                    ),
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

#[derive(Debug, Copy, Clone)]
struct Depth {
    max: u32,
    curr: u32,
}

impl Depth {
    fn inc(self) -> (Self, bool) {
        let Depth { max, curr } = self;
        let curr = curr + 1;
        (Depth { max, curr }, curr < max || max == 0)
    }
}
