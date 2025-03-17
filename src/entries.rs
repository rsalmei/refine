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
    depth: Depth,
    selector: Selector,
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

impl Entries {
    /// Reads all entries from a single directory.
    pub fn single(entry: Entry, depth: u32) -> Result<Self> {
        Self::new(vec![entry], depth, Filter::default())
    }

    /// Reads entries from the given directories, with the given filtering rules and recursion.
    pub fn new(dirs: Vec<Entry>, depth: u32, filter: Filter) -> Result<Self> {
        let selector = filter.try_into()?; // compile regexes and check for errors before anything else.

        if dirs.is_empty() {
            return Err(anyhow!("no valid paths given"));
        }

        Ok(Entries {
            dirs,
            depth: Depth::new(depth),
            selector,
        })
    }

    pub fn fetch(self, es: EntrySet) -> impl Iterator<Item = Entry> {
        let s = Rc::new(self.selector);
        self.dirs
            .into_iter()
            .flat_map(move |dir| entries(dir, self.depth, es, Rc::clone(&s)))
    }
}

#[derive(Debug, Copy, Clone)]
struct Depth {
    max: u32,
    curr: u32,
}

impl Depth {
    fn new(max: u32) -> Self {
        Depth { max, curr: 0 }
    }
    fn inc(self) -> (Self, bool) {
        let Depth { max, curr } = self;
        let curr = curr + 1;
        (Depth { max, curr }, curr < max || max == 0)
    }
}

fn entries(dir: Entry, d: Depth, es: EntrySet, s: Rc<Selector>) -> Box<dyn Iterator<Item = Entry>> {
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
                let (d, rec) = d.inc();
                match (entry.is_dir(), s.is_in(&entry), rec, es) {
                    (false, true, _, _) => Box::new(iter::once(entry)),
                    (true, false, true, _) => entries(entry, d, es, Rc::clone(&s)),
                    (true, true, false, DirsStop | DirsAndContent | ContentOverDirs) => {
                        Box::new(iter::once(entry))
                    }
                    (true, true, true, Files | ContentOverDirs) => {
                        entries(entry, d, es, Rc::clone(&s))
                    }
                    (true, true, true, DirsStop) => Box::new(iter::once(entry)),
                    (true, true, true, DirsAndContent) => Box::new(
                        iter::once(entry.clone()).chain(entries(entry, d, es, Rc::clone(&s))),
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
