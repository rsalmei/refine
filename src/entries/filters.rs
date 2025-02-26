use anyhow::{Context, Result, anyhow};
use clap::Args;
use clap::builder::NonEmptyStringValueParser;
use regex::Regex;

#[derive(Debug, Args, Default)]
pub struct Filters {
    /// Include only these files and directories; checked without extension.
    #[arg(short = 'i', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    include: Option<String>,
    /// Exclude these files and directories; checked without extension.
    #[arg(short = 'x', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    exclude: Option<String>,
    /// Include only these directories.
    #[arg(short = 'I', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    dir_in: Option<String>,
    /// Exclude these directories.
    #[arg(short = 'X', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    dir_ex: Option<String>,
    /// Include only these files; checked without extension.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    file_in: Option<String>,
    /// Exclude these files; checked without extension.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    file_ex: Option<String>,
    /// Include only these extensions.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    ext_in: Option<String>,
    /// Exclude these extensions.
    #[arg(long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    ext_ex: Option<String>,
}

#[derive(Debug)]
pub struct CFilters {
    pub include: Option<Regex>,
    pub exclude: Option<Regex>,
    pub dir_in: Option<Regex>,
    pub dir_ex: Option<Regex>,
    pub file_in: Option<Regex>,
    pub file_ex: Option<Regex>,
    pub ext_in: Option<Regex>,
    pub ext_ex: Option<Regex>,
    _private: (),
}

impl TryFrom<Filters> for CFilters {
    type Error = anyhow::Error;

    fn try_from(filters: Filters) -> Result<Self, Self::Error> {
        Ok(CFilters {
            include: compile(filters.include, "include")?,
            exclude: compile(filters.exclude, "exclude")?,
            dir_in: compile(filters.dir_in, "dir-in")?,
            dir_ex: compile(filters.dir_ex, "dir-ex")?,
            file_in: compile(filters.file_in, "file-in")?,
            file_ex: compile(filters.file_ex, "file-ex")?,
            ext_in: compile(filters.ext_in, "ext-in")?,
            ext_ex: compile(filters.ext_ex, "ext-ex")?,
            _private: (),
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
