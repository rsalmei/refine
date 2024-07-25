use crate::opt;
use crate::utils::{self, StripPos};
use anyhow::{Context, Result};
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
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
    #[arg(short, long, value_name = "{STR|REGEX}=STR", allow_hyphen_values = true, value_parser = utils::parse_key_value::<String, String>)]
    pub replace: Vec<(String, String)>,
    /// Allow changes in directories where clashes are detected.
    #[arg(short, long)]
    pub clashes: bool,
    /// Skip the confirmation prompt, useful for automation.
    #[arg(short, long)]
    pub yes: bool,
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

opt!(Rename);

pub fn run(mut medias: Vec<Media>) -> Result<()> {
    println!("=> Renaming files...\n");

    // step: apply strip rules.
    utils::strip_names(&mut medias, StripPos::Before, &opt().strip_before)?;
    utils::strip_names(&mut medias, StripPos::After, &opt().strip_after)?;
    utils::strip_names(&mut medias, StripPos::Exact, &opt().strip_exact)?;

    // step: apply replacement rules.
    for (k, v) in &opt().replace {
        let re =
            Regex::new(&format!("(?i){k}")).with_context(|| format!("compiling regex: {k:?}"))?;
        medias.iter_mut().for_each(|m| {
            if let Cow::Owned(s) = re.replace_all(&m.wname, v) {
                m.wname = s;
            }
        })
    }

    utils::user_aborted()?;

    // step: remove medias where the rules cleared the name.
    let total = medias.len();
    let mut warnings = utils::remove_cleared(&mut medias);

    // step: re-include extension in the names.
    medias
        .iter_mut()
        .filter(|m| !m.ext.is_empty())
        .try_for_each(|m| write!(m.wname, ".{}", m.ext))?;

    // step: disallow changes in directories where clashes are detected.
    medias.sort_unstable_by(|m, n| {
        (m.path.parent(), &m.path.file_name()).cmp(&(n.path.parent(), &n.path.file_name()))
    });
    medias
        .chunk_by_mut(|m, n| m.path.parent() == n.path.parent())
        .filter(|_| utils::is_running())
        .for_each(|g| {
            let path = g[0].path.parent().unwrap_or(Path::new("/")).to_owned();
            let mut clashes = HashMap::with_capacity(g.len());
            g.iter().for_each(|m| {
                clashes
                    .entry(&m.wname)
                    .or_insert_with(Vec::new)
                    .push(&m.path)
            });
            clashes.retain(|_, v| v.len() > 1);
            if !clashes.is_empty() {
                eprintln!("warning: names clash in: {}", path.display());
                let mut clashes = clashes.into_iter().collect::<Vec<_>>();
                clashes.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
                clashes.iter().for_each(|(k, v)| {
                    let list = v
                        .iter()
                        .map(|p| p.file_name().unwrap().to_str().unwrap())
                        .filter(|f| k != f)
                        .collect::<Vec<_>>();
                    warnings += list.len();
                    let exists = if v.len() != list.len() { " exists" } else { "" };
                    eprintln!("  > {} --> {k}{exists}", list.join(", "));
                });
                match opt().clashes {
                    false => g.iter_mut().for_each(|m| m.wname.clear()),
                    true => {
                        let keys = clashes.iter().map(|&(k, _)| k.clone()).collect::<Vec<_>>();
                        g.iter_mut()
                            .filter(|m| keys.contains(&m.wname))
                            .for_each(|m| m.wname.clear());
                    }
                }
            }
        });

    utils::user_aborted()?;

    // step: settle changes.
    let mut changes = medias
        .into_iter()
        .filter(|m| !m.wname.is_empty()) // new clash detection.
        .filter(|m| m.wname != m.path.file_name().unwrap().to_str().unwrap())
        .collect::<Vec<_>>();

    // step: display the results by parent directory.
    changes
        .chunk_by(|m, n| m.path.parent() == n.path.parent())
        .for_each(|g| {
            println!("{}:", g[0].path.parent().unwrap().display());
            g.iter().for_each(|m| {
                println!(
                    "  {} --> {}",
                    m.path.file_name().unwrap().to_str().unwrap(),
                    m.wname
                )
            });
        });

    // step: display receipt summary.
    if !changes.is_empty() || warnings > 0 {
        println!();
    }
    println!("total files: {total}");
    println!("  changes: {}", changes.len());
    println!("  warnings: {warnings}");
    if changes.is_empty() {
        return Ok(());
    }

    // step: apply changes, if the user agrees.
    if !opt().yes {
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

impl utils::WorkingName for Media {
    fn wname(&mut self) -> &mut String {
        &mut self.wname
    }
}

impl utils::PathWorkingName for Media {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl utils::NewNamePathWorkingName for Media {
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
