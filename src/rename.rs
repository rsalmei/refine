use crate::utils::{self, StripPos};
use anyhow::{Context, Result};
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::fmt::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct Rename {
    /// Remove from the start of the filename to this str; blanks are automatically removed.
    #[arg(short = 'b', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub strip_before: Vec<String>,
    /// Remove from this str to the end of the filename; blanks are automatically removed.
    #[arg(short = 'a', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub strip_after: Vec<String>,
    /// Remove all occurrences of this str in the filename; blanks are automatically removed.
    #[arg(short = 'e', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub strip_exact: Vec<String>,
    ///  Replace all occurrences of one str by another; applied in order and after the strip rules.
    #[arg(short, long, value_name = "STR|REGEX=STR", allow_hyphen_values = true, value_parser = utils::parse_key_value::<String, String>)]
    pub replace: Vec<(String, String)>,
    /// Skip the confirmation prompt, useful for automation.
    #[arg(short, long)]
    pub yes: bool,
}

fn opt() -> &'static Rename {
    match &super::args().cmd {
        super::Command::Rename(opt) => opt,
        _ => unreachable!(),
    }
}

#[derive(Debug)]
pub struct Media {
    /// The original path to the file.
    path: PathBuf,
    /// The working copy of the name, where the rules are applied.
    wname: String,
    /// A cached version of the file extension.
    ext: &'static str,
}

pub fn run(mut medias: Vec<Media>) -> Result<()> {
    println!("=> Renaming files...\n");

    // step: apply strip rules.
    utils::strip_names(&mut medias, StripPos::Before, &opt().strip_before)?;
    utils::strip_names(&mut medias, StripPos::After, &opt().strip_after)?;
    utils::strip_names(&mut medias, StripPos::Exact, &opt().strip_exact)?;

    // step: apply replace rules.
    replace_names(&mut medias)?;

    utils::user_aborted()?;

    // step: remove medias where the rules cleared the name.
    let total = medias.len();
    let (mut medias, mut cleared) = medias
        .into_iter()
        .partition::<Vec<_>, _>(|m| !m.wname.is_empty());
    cleared.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    cleared.iter().for_each(|m| {
        eprintln!("warning: rules cleared name: {}", m.path.display());
    });

    // step: settle changes, and display the results.
    medias
        .iter_mut()
        .filter(|m| !m.ext.is_empty())
        .try_for_each(|m| write!(m.wname, ".{}", m.ext))?;
    medias.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    let mut changes = medias
        .into_iter()
        .filter(|m| m.wname != m.path.file_name().unwrap().to_str().unwrap())
        .inspect(|m| {
            println!("{} --> {}", m.path.display(), m.wname);
        })
        .collect::<Vec<_>>();

    // step: display receipt summary.
    if !changes.is_empty() || !cleared.is_empty() {
        println!();
    }
    println!("total files: {total}");
    println!("  changes: {}", changes.len());

    // step: apply changes, if the user agrees.
    if !changes.is_empty() && !opt().yes {
        utils::prompt_yes_no("apply changes?")?;
    }
    utils::rename_consuming(&mut changes);
    if changes.is_empty() {
        println!("done");
        return Ok(());
    }
    println!("found {} errors", changes.len());
    Ok(())
}

fn replace_names(medias: &mut [Media]) -> Result<()> {
    for (k, v) in &opt().replace {
        let re =
            Regex::new(&format!("(?i){k}")).with_context(|| format!("compiling regex: {k:?}"))?;
        medias.iter_mut().for_each(|m| {
            if let Cow::Owned(s) = re.replace_all(&m.wname, v) {
                m.wname = s;
            }
        })
    }
    Ok(())
}

impl utils::WorkingName for Media {
    fn name(&mut self) -> &mut String {
        &mut self.wname
    }
}

impl utils::Rename for Media {
    fn path(&self) -> &Path {
        &self.path
    }
    fn new_name(&self) -> &str {
        &self.wname
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let (name, ext) = utils::file_stem_ext(&path)?;
        Ok(Media {
            wname: name.trim().to_owned(),
            ext: utils::intern(ext),
            path,
        })
    }
}
