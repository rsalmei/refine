mod entry;
mod filter;
mod sequence;

use crate::utils;
use anyhow::{Result, anyhow};
pub use entry::*;
pub use filter::*;
use std::iter;
use std::path::PathBuf;
use std::rc::Rc;

/// The object that fetches and filters entries from multiple directories.
#[derive(Debug)]
pub struct Entries {
    /// Effective input paths to scan, after deduplication and checking.
    dirs: Vec<Entry>,
    shallow: bool,
    selector: Selector,
    /// Warnings that were encountered while parsing the input.
    warnings: Warnings,
}

#[derive(Debug)]
pub struct Warnings {
    /// Whether there were missing paths in the input.
    pub missing: bool,
}

/// Denotes the set of entry types a command will process.
#[derive(Debug, Copy, Clone)]
pub enum EntrySet {
    /// Only files.
    Files,
    /// Directories alone or files, whatever matches.
    DirOrFiles,
    /// Both directories and its contents chained.
    DirAndFiles,
}

impl Entries {
    /// Reads all entries from a single directory, non-recursively.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Result<Self> {
        Self::new(vec![dir.into()], true, Filter::default())
    }

    /// Reads entries from the given directories, with the given filtering rules and recursion.
    pub fn new(dirs: Vec<PathBuf>, shallow: bool, f: Filter) -> Result<Self> {
        let selector = f.try_into()?; // compile regexes and check for errors before anything else.

        let mut dirs = match dirs.is_empty() {
            false => dirs,            // lists files from the given paths,
            true => vec![".".into()], // or the current directory if no paths are given.
        };
        let n = dirs.len();
        dirs.sort_unstable();
        dirs.dedup();
        if n != dirs.len() {
            eprintln!("warning: {} duplicated directories ignored", n - dirs.len());
        }

        let (dirs, missing) = dirs
            .into_iter()
            .map(Entry::try_from)
            .inspect(|res| {
                if let Err(err) = res {
                    eprintln!("warning: invalid entry: {err}");
                }
            })
            .flatten()
            .partition::<Vec<_>, _>(|p| p.is_dir());
        missing
            .iter()
            .for_each(|p| eprintln!("warning: directory not found: {}", p.display()));
        if dirs.is_empty() {
            return Err(anyhow!("no valid paths given"));
        }

        Ok(Entries {
            dirs,
            shallow,
            selector,
            warnings: Warnings {
                missing: !missing.is_empty(),
            },
        })
    }

    pub fn warnings(&self) -> &Warnings {
        &self.warnings
    }

    pub fn fetch(self, es: EntrySet) -> impl Iterator<Item = Entry> {
        let es = (!self.shallow).then_some(es);
        let s = Rc::new(self.selector);
        self.dirs
            .into_iter()
            .flat_map(move |dir| entries(dir, es, Rc::clone(&s)))
    }
}

fn entries(dir: Entry, es: Option<EntrySet>, s: Rc<Selector>) -> Box<dyn Iterator<Item = Entry>> {
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
                match (entry.is_dir(), s.is_included(&entry), es) {
                    (false, true, _) => Box::new(iter::once(entry)),
                    (true, false, Some(_)) => entries(entry, es, Rc::clone(&s)),
                    (true, true, Some(Files)) => entries(entry, es, Rc::clone(&s)),
                    (true, true, Some(DirOrFiles)) => Box::new(iter::once(entry)),
                    (true, true, Some(DirAndFiles)) => {
                        Box::new(iter::once(entry.clone()).chain(entries(entry, es, Rc::clone(&s))))
                    }
                    _ => Box::new(iter::empty()),
                }
            }),
        ),
        Err(err) => {
            eprintln!("error: read dir {dir:?}: {err}");
            Box::new(iter::empty())
        }
    }
}
