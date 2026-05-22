use crate::entries::Entry;
use anyhow::{Context, Result};
use clap::Args;
use clap::builder::NonEmptyStringValueParser;
use regex::Regex;

/// A set of rules that allow the user to specify which files and directories to include or exclude.
#[derive(Debug, Args)]
#[command(next_help_heading = Some("Fetch"))]
pub struct FilterArgs {
    /// Include only files.
    #[arg(short = 'F', long, conflicts_with = "only_dirs")]
    only_files: bool,
    /// Include only directories.
    #[arg(short = 'D', long, conflicts_with = "only_files")]
    only_dirs: bool,

    // general rules that apply to all entries.
    /// Include files, directories, and paths that matches this.
    #[arg(short = 'i', long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    inc: Vec<String>,
    /// Exclude files, directories, and paths that matches this.
    #[arg(short = 'x', long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    exc: Vec<String>,

    // directory rules.
    /// Include only these directory names.
    #[arg(short = 'I', long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    dir: Vec<String>,
    /// Exclude these directory names.
    #[arg(short = 'X', long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    dir_ex: Vec<String>,

    // path rules.
    /// Include only these paths.
    #[arg(long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    path: Vec<String>,
    /// Exclude these paths.
    #[arg(long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    path_ex: Vec<String>,

    // filename rules.
    /// Include only these filenames.
    #[arg(long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    file: Vec<String>,
    /// Exclude these filenames.
    #[arg(long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    file_ex: Vec<String>,

    // extension rules.
    /// Include only these extensions.
    #[arg(long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    ext: Vec<String>,
    /// Exclude these extensions.
    #[arg(long, value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new(), value_delimiter = ',')]
    ext_ex: Vec<String>,
}

/// A constraint that applies inclusion and/or exclusion regexes to a string value.
///
/// Each variant encodes exactly which regexes are active:
/// - `Include`: only match values that match the regex.
/// - `Exclude`: only reject values that match the regex.
/// - `Both`: match values that match the inclusion AND don't match the exclusion.
#[derive(Debug)]
enum Constraint {
    Include(Regex),
    Exclude(Regex),
    Both(Regex, Regex),
}

/// The engine that applies the [FilterArgs] rules to a collection of entries.
#[derive(Debug, Default)]
pub struct Filter {
    only_files: bool,
    only_dirs: bool,
    all: Option<Constraint>,
    dir: Option<Constraint>,
    path: Option<Constraint>,
    file: Option<Constraint>,
    ext: Option<Constraint>,
}

impl Filter {
    pub fn is_in(&self, entry: &Entry) -> bool {
        self.is_included(entry).unwrap_or_default()
    }

    fn is_included(&self, entry: &Entry) -> Option<bool> {
        let (stem, ext) = entry.filename_parts();
        (!stem.starts_with('.')).then_some(())?; // exclude hidden files and directories.

        let ret = if entry.is_dir() {
            // cheap dir-specific checks first; skip entry.file_name()/to_str() when unconstrained.
            self.dir.as_ref().is_none_or(|r| r.is_match(entry.file_name()))
                && self.path.as_ref().is_none_or(|r| r.is_match(entry.to_str()))
                && !self.only_files
                // only build parent and the full-path string if the all rule is active.
                && match &self.all {
                    None => true,
                    Some(r) => {
                        let parent = entry.parent()?;
                        r.is_match(&format!("{}{stem}", parent.to_str()))
                    }
                }
        } else {
            // cheap file-specific checks first; stem/ext are already extracted above.
            self.file.as_ref().is_none_or(|r| r.is_match(stem))
                && self.ext.as_ref().is_none_or(|r| r.is_match(ext))
                && !self.only_dirs
                // only allocate a parent entry if any parent-dependent rule is active.
                && (self.all.is_none() && self.dir.is_none() && self.path.is_none()
                    || {
                        let parent = entry.parent()?;
                        self.all.as_ref().is_none_or(|r| r.is_match(&format!("{}{stem}", parent.to_str())))
                            && self.dir.as_ref().is_none_or(|r| r.is_match(parent.file_name()))
                            && self.path.as_ref().is_none_or(|r| r.is_match(parent.to_str()))
                    })
        };
        Some(ret)
    }
}

impl Constraint {
    /// Check if a value passes this constraint.
    fn is_match(&self, s: &str) -> bool {
        match self {
            Constraint::Include(re) => re.is_match(s),
            Constraint::Exclude(re) => !re.is_match(s),
            Constraint::Both(re_in, re_ex) => re_in.is_match(s) && !re_ex.is_match(s),
        }
    }
}

impl TryFrom<FilterArgs> for Filter {
    type Error = anyhow::Error;

    fn try_from(s: FilterArgs) -> Result<Self> {
        Ok(Filter {
            only_files: s.only_files,
            only_dirs: s.only_dirs,
            all: build((s.inc, "inc"), (s.exc, "all-ex"))?,
            dir: build((s.dir, "dir"), (s.dir_ex, "dir-ex"))?,
            path: build((s.path, "path"), (s.path_ex, "path-ex"))?,
            file: build((s.file, "file"), (s.file_ex, "file-ex"))?,
            ext: build((s.ext, "ext"), (s.ext_ex, "ext-ex"))?,
        })
    }
}

type Rule<'a> = (Vec<String>, &'a str);

/// Build a rule from an include/exclude pair, returning `None` if both are absent.
fn build((_in, p_in): Rule, (_ex, p_ex): Rule) -> Result<Option<Constraint>> {
    Ok(match (compile(_in, p_in)?, compile(_ex, p_ex)?) {
        (Some(re_in), None) => Some(Constraint::Include(re_in)),
        (None, Some(re_ex)) => Some(Constraint::Exclude(re_ex)),
        (Some(re_in), Some(re_ex)) => Some(Constraint::Both(re_in, re_ex)),
        (None, None) => None,
    })
}

/// Compile an optional regular expression (case-insensitive).
fn compile(value: Vec<String>, param: &str) -> Result<Option<Regex>> {
    (!value.is_empty())
        .then(|| {
            let r = value
                .iter()
                .map(|v| format!("(?:{v})")) // wrap each regex in a non-capturing group to preserve the intended precedence when joining them.
                .collect::<Vec<_>>()
                .join("|");
            Regex::new(&format!("(?i){r}")).with_context(|| format!("invalid --{param}"))
        })
        .transpose()
}
