use super::Entry;
use anyhow::{Context, Result, anyhow};
use clap::Args;
use clap::builder::NonEmptyStringValueParser;
use regex::Regex;

/// A set of rules that allow the user to specify which files and directories to include or exclude.
#[derive(Debug, Args)]
pub struct Filter {
    /// Include only files.
    #[arg(short = 'F', long, global = true, conflicts_with = "only_dirs", help_heading = Some("Fetch"))]
    only_files: bool,
    /// Include only directories.
    #[arg(short = 'D', long, global = true, conflicts_with = "only_files", help_heading = Some("Fetch"))]
    only_dirs: bool,
    /// Include everything that matches this (regardless of files or directories/paths).
    #[arg(short = 'i', long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    all_in: Option<String>,
    /// Include only these current directories.
    #[arg(short = 'I', long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    dir_in: Option<String>,
    /// Include only these paths.
    #[arg(long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    path_in: Option<String>,
    /// Include only these filenames.
    #[arg(long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    file_in: Option<String>,
    /// Include only these extensions.
    #[arg(long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    ext_in: Option<String>,
    /// Exclude everything that matches this (regardless of files or directories/paths).
    #[arg(short = 'x', long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    all_ex: Option<String>,
    /// Exclude these current directories.
    #[arg(short = 'X', long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    dir_ex: Option<String>,
    /// Exclude these paths.
    #[arg(long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    path_ex: Option<String>,
    /// Exclude these filenames.
    #[arg(long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    file_ex: Option<String>,
    /// Exclude these extensions.
    #[arg(long, global = true, help_heading = Some("Fetch"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    ext_ex: Option<String>,
}

/// The engine that applies the [Filter] rules to a collection of entries.
#[derive(Debug, Default)]
pub struct FilterRules {
    only_files: bool,
    only_dirs: bool,
    all: Constraint,
    dir: Constraint,
    path: Constraint,
    file: Constraint,
    ext: Constraint,
}

impl FilterRules {
    pub fn is_in(&self, entry: &Entry) -> bool {
        self.is_included(entry).unwrap_or_default()
    }

    fn is_included(&self, entry: &Entry) -> Option<bool> {
        let (stem, ext) = entry.filename_parts();
        (!stem.starts_with('.')).then_some(())?; // exclude hidden files and directories.

        let parent = entry.parent()?;
        let full = format!("{}{stem}", parent.to_str()); // generate the full path without extension.
        let ret = self.all.is_match(&full)
            && match entry.is_dir() {
                true => {
                    self.dir.is_match(entry.file_name()) // entry is a directory.
                        && self.path.is_match(entry.to_str()) // the str is the full path.
                        && !self.only_files
                }
                false => {
                    self.file.is_match(stem)
                        && self.ext.is_match(ext)
                        && self.dir.is_match(parent.file_name())
                        && self.path.is_match(parent.to_str())
                        && !self.only_dirs
                }
            };
        Some(ret)
    }
}

/// A pair of regexes that check strings for inclusion and exclusion.
#[derive(Debug, Default)]
pub struct Constraint {
    re_in: Option<Regex>,
    re_ex: Option<Regex>,
}

impl Constraint {
    fn is_match(&self, s: &str) -> bool {
        self.re_ex.as_ref().is_none_or(|re_ex| !re_ex.is_match(s))
            && self.re_in.as_ref().is_none_or(|re_in| re_in.is_match(s))
    }
}

type Param<'a> = (Option<String>, &'a str);

impl TryFrom<[Param<'_>; 2]> for Constraint {
    type Error = anyhow::Error;

    fn try_from([(re_in, p_in), (re_ex, p_ex)]: [Param; 2]) -> Result<Self> {
        Ok(Self {
            re_in: compile(re_in, p_in)?,
            re_ex: compile(re_ex, p_ex)?,
        })
    }
}

impl TryFrom<Filter> for FilterRules {
    type Error = anyhow::Error;

    fn try_from(s: Filter) -> Result<Self, Self::Error> {
        Ok(FilterRules {
            only_files: s.only_files,
            only_dirs: s.only_dirs,
            all: [(s.all_in, "all-in"), (s.all_ex, "all-ex")].try_into()?,
            dir: [(s.dir_in, "dir-in"), (s.dir_ex, "dir-ex")].try_into()?,
            path: [(s.path_in, "path-in"), (s.path_ex, "path-ex")].try_into()?,
            file: [(s.file_in, "file-in"), (s.file_ex, "file-ex")].try_into()?,
            ext: [(s.ext_in, "ext-in"), (s.ext_ex, "ext-ex")].try_into()?,
        })
    }
}

// Compile an optional regular expression (case-insensitive).
fn compile(value: Option<String>, param: &str) -> Result<Option<Regex>> {
    let compiler = |r| {
        Regex::new(&format!("(?i){r}"))
            .with_context(|| format!("compiling regex: {r:?}"))
            .map_err(|err| anyhow!("error: invalid --{param}: {err:?}"))
    };
    value.map(compiler).transpose()
}
