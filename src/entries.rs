use crate::utils;
use anyhow::{anyhow, Result};
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::iter;
use std::path::{Path, PathBuf};
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

/// Denotes which kind of entries will be collected.
#[derive(Debug, Copy, Clone)]
pub enum EntrySet {
    /// Only files.
    Files,
    /// Either directories or files, in this order.
    Either,
    /// Both directories and files, in this order.
    Both,
}

#[derive(Debug, Copy, Clone)]
enum RecurseMode {
    Recurse(EntrySet),
    Shallow,
}

#[derive(Debug)]
pub struct Entries {
    dirs: Vec<PathBuf>,
    shallow: bool,
    /// Used to determine whether there were missing directories in the input.
    missing_dirs: bool,
}

impl Entries {
    pub(super) fn new(mut dirs: Vec<PathBuf>, filters: Filters) -> Result<Entries> {
        parse_input_regexes(&filters)?;

        let prev = dirs.len();
        dirs.sort_unstable();
        dirs.dedup();
        if prev != dirs.len() {
            eprintln!("warning: {} duplicated path(s) ignored", prev - dirs.len());
        }

        let prev = dirs.len();
        dirs.retain(|p| p.is_dir());
        if dirs.is_empty() {
            return Err(anyhow!("no valid paths given"));
        }

        Ok(Entries {
            missing_dirs: prev != dirs.len(), // use dirs before moving it below.
            dirs,
            shallow: filters.shallow,
        })
    }

    pub(super) fn fetch(&self, kind: EntrySet) -> impl Iterator<Item = PathBuf> + '_ {
        let rm = match self.shallow {
            true => RecurseMode::Shallow,
            false => RecurseMode::Recurse(kind),
        };
        self.dirs
            .iter()
            .flat_map(move |p| entries(p.to_owned(), rm))
    }

    pub fn missing_dirs(&self) -> bool {
        self.missing_dirs
    }
}

macro_rules! re_input {
    ($($re:ident, $name:ident);+ $(;)?) => {
        $( static $re: OnceLock<Regex> = OnceLock::new(); )+
        fn parse_input_regexes(filters: &Filters) -> Result<()> {
            $( utils::set_re(&filters.$name, &$re, stringify!($name))?; )+
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

fn entries(dir: PathBuf, rm: RecurseMode) -> Box<dyn Iterator<Item = PathBuf>> {
    fn is_included(path: &Path) -> Option<bool> {
        fn is_match(s: &str, re_in: Option<&Regex>, re_ex: Option<&Regex>) -> bool {
            re_ex.map_or(true, |re_ex| !re_ex.is_match(s))
                && re_in.map_or(true, |re_in| re_in.is_match(s))
        }

        let (name, ext) = utils::filename_parts(path).ok()?; // discards invalid UTF-8 names.
        (!name.starts_with('.')).then_some(())?; // exclude hidden files and directories.

        (is_match(name, RE_IN.get(), RE_EX.get()) // applied to both files and directories.
            && is_match(ext, RE_EIN.get(), RE_EEX.get())
            && match path.is_dir() {
                true => is_match(path.to_str()?, RE_DIN.get(), RE_DEX.get()),
                false => is_match(path.parent()?.to_str()?, RE_DIN.get(), RE_DEX.get())
                    && is_match(name, RE_FIN.get(), RE_FEX.get()),
            })
        .into()
    }

    if !utils::is_running() {
        return Box::new(iter::empty());
    }

    // this does allow hidden directories, if the user directly asks for them.
    match std::fs::read_dir(&dir) {
        Ok(rd) => Box::new(
            rd.inspect(move |r| {
                if let Err(err) = r {
                    eprintln!("error: read entry {}: {err}", dir.display());
                }
            })
            .flatten()
            .flat_map(move |de| {
                let path = de.path();
                use {EntrySet::*, RecurseMode::*};
                match (path.is_dir(), is_included(&path), rm) {
                    (false, Some(true), _) => Box::new(iter::once(path)),
                    (true, Some(false), Recurse(_)) => entries(path, rm),
                    (true, Some(true), Recurse(Files)) => entries(path, rm),
                    (true, Some(true), Recurse(Either)) => Box::new(iter::once(path)),
                    (true, Some(true), Recurse(Both)) => {
                        Box::new(iter::once(path.to_owned()).chain(entries(path, rm)))
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
