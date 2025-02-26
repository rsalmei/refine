mod entry;
mod filters;

use crate::utils;
use anyhow::{Result, anyhow};
pub use entry::*;
pub use filters::*;
use regex::Regex;
use std::iter;
use std::path::PathBuf;
use std::rc::Rc;

/// The object that fetches and filters entries from multiple directories.
#[derive(Debug)]
pub struct Entries {
    /// Effective input paths to scan, after deduplication and checking.
    dirs: Vec<Entry>,
    /// Compiled regexes for filtering entries.
    filters: CFilters,
    /// Whether to scan subdirectories of each directory.
    shallow: bool,
    /// Warnings that were encountered while parsing the input.
    warnings: Warnings,
}

#[derive(Debug)]
pub struct Warnings {
    /// Whether there were missing paths in the input.
    pub missing: bool,
}

/// Denotes which kind of entries should be included.
#[derive(Debug, Copy, Clone)]
pub enum EntryKinds {
    /// Only files.
    Files,
    /// Either directories or its contents.
    Either,
    /// Both directories and its contents, in this order.
    Both,
}

impl Entries {
    /// Reads all entries from a single directory, non-recursively.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Result<Self> {
        Self::with_filters(vec![dir.into()], Filters::default(), false)
    }

    /// Reads entries from the given directories, with the given filtering rules and recursion.
    pub fn with_filters(dirs: Vec<PathBuf>, filters: Filters, shallow: bool) -> Result<Self> {
        let filters = filters.try_into()?; // compile regexes and check for errors before anything else.

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
            filters,
            shallow,
            warnings: Warnings {
                missing: !missing.is_empty(),
            },
        })
    }

    pub fn warnings(&self) -> &Warnings {
        &self.warnings
    }

    pub fn fetch(self, kinds: EntryKinds) -> impl Iterator<Item = Entry> {
        let kind = (!self.shallow).then_some(kinds);
        let cf = Rc::new(self.filters);
        self.dirs
            .into_iter()
            .flat_map(move |p| entries(p, kind, Rc::clone(&cf)))
    }
}

fn entries(dir: Entry, k: Option<EntryKinds>, cf: Rc<CFilters>) -> Box<dyn Iterator<Item = Entry>> {
    fn is_included(entry: &Entry, cf: &CFilters) -> Option<bool> {
        fn is_match(s: &str, re_in: Option<&Regex>, re_ex: Option<&Regex>) -> bool {
            re_ex.is_none_or(|re_ex| !re_ex.is_match(s))
                && re_in.is_none_or(|re_in| re_in.is_match(s))
        }

        let (stem, ext) = entry.filename_parts();
        (!stem.starts_with('.')).then_some(())?; // exclude hidden files and directories.

        (is_match(stem, cf.include.as_ref(), cf.exclude.as_ref()) // applied to both files and directories.
            && is_match(ext, cf.ext_in.as_ref(), cf.ext_ex.as_ref())
            && match entry.is_dir() {
                true => is_match(entry.to_str()?, cf.dir_in.as_ref(), cf.dir_ex.as_ref()),
                false => is_match(entry.parent()?.to_str()?, cf.dir_in.as_ref(), cf.dir_ex.as_ref())
                    && is_match(stem, cf.file_in.as_ref(), cf.file_ex.as_ref()),
            })
        .into()
    }

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
                use EntryKinds::*;
                match (entry.is_dir(), is_included(&entry, &cf), k) {
                    (false, Some(true), _) => Box::new(iter::once(entry)),
                    (true, Some(false), Some(_)) => entries(entry, k, Rc::clone(&cf)),
                    (true, Some(true), Some(Files)) => entries(entry, k, Rc::clone(&cf)),
                    (true, Some(true), Some(Either)) => Box::new(iter::once(entry)),
                    (true, Some(true), Some(Both)) => {
                        Box::new(iter::once(entry.clone()).chain(entries(entry, k, Rc::clone(&cf))))
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
