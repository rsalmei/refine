mod entry;

use crate::utils;
use anyhow::{anyhow, Context, Result};
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
pub use entry::*;
use regex::Regex;
use std::iter;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Args)]
pub struct Filters {
    /// Include only these files and directories; checked without extension.
    #[arg(short = 'i', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub include: Option<String>,
    /// Exclude these files and directories; checked without extension.
    #[arg(short = 'x', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub exclude: Option<String>,
    /// Include only these directories.
    #[arg(short = 'I', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub dir_in: Option<String>,
    /// Exclude these directories.
    #[arg(short = 'X', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub dir_ex: Option<String>,
    /// Include only these files; checked without extension.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub file_in: Option<String>,
    /// Exclude these files; checked without extension.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub file_ex: Option<String>,
    /// Include only these extensions.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub ext_in: Option<String>,
    /// Exclude these extensions.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub ext_ex: Option<String>,
    /// Do not recurse into subdirectories.
    #[arg(short = 'w', long, global = true, help_heading = Some("Global"))]
    pub shallow: bool,
}

/// The object that fetches and filters entries from multiple directories.
#[derive(Debug)]
pub struct Entries {
    /// Effective input paths to scan, after deduplication and checking.
    dirs: Vec<Entry>,
    /// Whether there were missing paths in the input.
    pub missing_dirs: bool,
    shallow: bool,
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
    pub fn new(mut dirs: Vec<PathBuf>, filters: Filters) -> Result<Entries> {
        parse_input_regexes(&filters)?;

        let n = dirs.len();
        dirs.sort_unstable();
        dirs.dedup();
        if n != dirs.len() {
            eprintln!("warning: {} duplicated directories ignored", n - dirs.len());
        }

        let (dirs, errs) = dirs
            .into_iter()
            .map(|p| Entry::try_from(p))
            .inspect(|res| {
                if let Err(err) = res {
                    eprintln!("warning: directory not found: {err}");
                }
            })
            .flatten()
            .partition::<Vec<_>, _>(|p| p.is_dir());
        errs.iter()
            .for_each(|p| eprintln!("warning: directory not found: {}", p.display()));
        if dirs.is_empty() {
            return Err(anyhow!("no valid paths given"));
        }

        Ok(Entries {
            dirs,
            shallow: filters.shallow,
            missing_dirs: !errs.is_empty(),
        })
    }

    pub fn fetch(self, kinds: EntryKinds) -> impl Iterator<Item = Entry> {
        let kind = (!self.shallow).then_some(kinds);
        self.dirs.into_iter().flat_map(move |p| entries(p, kind))
    }
}

// Set an optional regular expression into a OnceLock (case-insensitive).
fn set_regex(var: &OnceLock<Regex>, val: &Option<String>, param: &str) -> Result<()> {
    match val {
        None => Ok(()),
        Some(s) => match Regex::new(&format!("(?i){s}"))
            .with_context(|| format!("compiling regex: {s:?}"))
        {
            Ok(re) => {
                var.set(re).unwrap();
                Ok(())
            }
            Err(err) => Err(anyhow!("error: invalid --{param}: {err:?}")),
        },
    }
}

macro_rules! re_input {
    ($($re:ident, $param:ident);+ $(;)?) => {
        $( static $re: OnceLock<Regex> = OnceLock::new(); )+
        fn parse_input_regexes(filters: &Filters) -> Result<()> {
            $( set_regex(&$re, &filters.$param, stringify!($param))?; )+
            Ok(())
        }
    };
}
re_input!(
    RE_IN, include; RE_EX, exclude; // general include and exclude (both files and directories).
    RE_DIN, dir_in; RE_DEX, dir_ex; // directory include and exclude.
    RE_FIN, file_in; RE_FEX, file_ex; // file include and exclude.
    RE_EIN, ext_in; RE_EEX, ext_ex; // extension include and exclude.
);

fn entries(dir: Entry, kind: Option<EntryKinds>) -> Box<dyn Iterator<Item = Entry>> {
    fn is_included(entry: &Entry) -> Option<bool> {
        fn is_match(s: &str, re_in: Option<&Regex>, re_ex: Option<&Regex>) -> bool {
            re_ex.map_or(true, |re_ex| !re_ex.is_match(s))
                && re_in.map_or(true, |re_in| re_in.is_match(s))
        }

        let (stem, ext) = entry.filename_parts();
        (!stem.starts_with('.')).then_some(())?; // exclude hidden files and directories.

        (is_match(stem, RE_IN.get(), RE_EX.get()) // applied to both files and directories.
            && is_match(ext, RE_EIN.get(), RE_EEX.get())
            && match entry.is_dir() {
                true => is_match(entry.to_str()?, RE_DIN.get(), RE_DEX.get()),
                false => is_match(entry.parent()?.to_str()?, RE_DIN.get(), RE_DEX.get())
                    && is_match(stem, RE_FIN.get(), RE_FEX.get()),
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
                match (entry.is_dir(), is_included(&entry), kind) {
                    (false, Some(true), _) => Box::new(iter::once(entry)),
                    (true, Some(false), Some(_)) => entries(entry, kind),
                    (true, Some(true), Some(Files)) => entries(entry, kind),
                    (true, Some(true), Some(Either)) => Box::new(iter::once(entry)),
                    (true, Some(true), Some(Both)) => {
                        Box::new(iter::once(entry.clone()).chain(entries(entry, kind)))
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
