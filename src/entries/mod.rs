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
    /// Contents are listed while recursing, and directories at the max depth.
    ContentOverDirs, // list
}

pub enum Recurse {
    Full,
    Shallow,
    UpTo(u32),
}

impl Entries {
    /// Reads all entries from a single directory.
    pub fn single(entry: impl Into<Entry>, recurse: Recurse) -> Self {
        Self::new(vec![entry.into()], recurse, Filter::default()).unwrap() // can't fail.
    }

    /// Reads entries from the given directories, with the given filtering rules and recursion.
    pub fn new(dirs: Vec<Entry>, recurse: Recurse, filter: Filter) -> Result<Self> {
        let selector = filter.try_into()?; // compile regexes and check for errors before anything else.

        if dirs.is_empty() {
            return Err(anyhow!("no valid paths given"));
        }

        Ok(Entries {
            dirs,
            depth: recurse.into(),
            selector: Rc::new(selector),
        })
    }

    pub fn fetch(self, es: EntrySet) -> impl Iterator<Item = Entry> {
        self.dirs
            .into_iter()
            .flat_map(move |dir| entries(dir, self.depth, es, Rc::clone(&self.selector)))
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
            .map(move |de| de.file_name().to_str().map(|s| dir.join(s)).ok_or(de))
            .inspect(|res| {
                if let Err(de) = res {
                    eprintln!("error: no UTF-8 name: {de:?}");
                }
            })
            .flatten()
            .flat_map(move |entry| {
                use EntrySet::*;
                let (d, rec) = d.inc();
                match (entry.is_dir(), s.is_in(&entry), rec, es) {
                    (false, true, _, _) => Box::new(iter::once(entry)),
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
