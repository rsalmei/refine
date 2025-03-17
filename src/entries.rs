mod entry;
mod filter;
mod sequence;

use crate::utils;
use anyhow::{Result, anyhow};
pub use entry::*;
pub use filter::*;
use std::iter;
use std::rc::Rc;

/// The object that fetches and filters entries from multiple directories.
#[derive(Debug)]
pub struct Entries {
    /// Effective input paths to scan, after deduplication and checking.
    dirs: Vec<Entry>,
    recurse: Recurse,
    selector: Rc<Selector>,
}

/// Denotes the set of entry types a command will process.
#[derive(Debug, Copy, Clone)]
pub enum EntrySet {
    /// Only files.
    Files, // dupes, probe, and rebuild.
    /// Directories stop recursion because the dir itself is the output.
    DirsStop, // join.
    /// Directories are chained with their content.
    DirsAndContent, // rename.
    /// Contents are listed when recursing, or directories otherwise.
    ContentOverDirs, // list
}

pub enum Depth {
    Unlimited,
    Shallow,
    Max(u32),
}

impl Entries {
    /// Reads all entries from a single directory.
    pub fn single(entry: impl Into<Entry>, depth: Depth) -> Self {
        Self::new(vec![entry.into()], depth, Filter::default()).unwrap() // can't fail.
    }

    /// Reads entries from the given directories, with the given filtering rules and recursion.
    pub fn new(dirs: Vec<Entry>, depth: Depth, filter: Filter) -> Result<Self> {
        let selector = filter.try_into()?; // compile regexes and check for errors before anything else.

        if dirs.is_empty() {
            return Err(anyhow!("no valid paths given"));
        }

        Ok(Entries {
            dirs,
            recurse: depth.into(),
            selector: Rc::new(selector),
        })
    }

    pub fn fetch(self, es: EntrySet) -> impl Iterator<Item = Entry> {
        self.dirs
            .into_iter()
            .flat_map(move |dir| entries(dir, self.recurse, es, Rc::clone(&self.selector)))
    }
}

fn entries(
    dir: Entry,
    r: Recurse,
    es: EntrySet,
    s: Rc<Selector>,
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
            .map(|de| Entry::try_from(de.path()))
            .inspect(|res| {
                if let Err(err) = res {
                    eprintln!("error: entry: {err}");
                }
            })
            .flatten()
            .flat_map(move |entry| {
                use EntrySet::*;
                let (r, rec) = r.inc();
                match (entry.is_dir(), s.is_in(&entry), rec, es) {
                    (false, true, _, _) => Box::new(iter::once(entry)),
                    (true, false, true, _) => entries(entry, r, es, Rc::clone(&s)),
                    (true, true, false, DirsStop | DirsAndContent | ContentOverDirs) => {
                        Box::new(iter::once(entry))
                    }
                    (true, true, true, Files | ContentOverDirs) => {
                        entries(entry, r, es, Rc::clone(&s))
                    }
                    (true, true, true, DirsStop) => Box::new(iter::once(entry)),
                    (true, true, true, DirsAndContent) => Box::new(
                        iter::once(entry.clone()).chain(entries(entry, r, es, Rc::clone(&s))),
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

impl From<u32> for Depth {
    fn from(d: u32) -> Self {
        match d {
            0 => Depth::Unlimited,
            1 => Depth::Shallow,
            _ => Depth::Max(d),
        }
    }
}

impl From<Depth> for Recurse {
    fn from(d: Depth) -> Self {
        match d {
            Depth::Unlimited => Recurse { max: 0, curr: 0 },
            Depth::Shallow => Recurse { max: 1, curr: 0 },
            Depth::Max(d) => Recurse { max: d, curr: 0 },
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct Recurse {
    max: u32,
    curr: u32,
}

impl Recurse {
    fn inc(self) -> (Self, bool) {
        let Recurse { max, curr } = self;
        let curr = curr + 1;
        (Recurse { max, curr }, curr < max || max == 0)
    }
}
