use crate::commands::Refine;
use crate::entries::EntryKind;
use crate::utils;
use anyhow::{Context, Result};
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct Rename {
    /// Remove from the start of the name to this str; blanks are automatically removed.
    #[arg(short = 'b', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_before: Vec<String>,
    /// Remove from this str to the end of the name; blanks are automatically removed.
    #[arg(short = 'a', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_after: Vec<String>,
    /// Remove all occurrences of this str in the name; blanks are automatically removed.
    #[arg(short = 'e', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_exact: Vec<String>,
    ///  Replace all occurrences of one str with another; applied in order and after the strip rules.
    #[arg(short = 'r', long, value_name = "{STR|REGEX}=STR", allow_hyphen_values = true, value_parser = utils::parse_key_value::<String, String>)]
    replace: Vec<(String, String)>,
    /// Allow changes in directories where clashes are detected.
    #[arg(short = 'c', long)]
    clashes: bool,
    /// Skip the confirmation prompt, useful for automation.
    #[arg(short = 'y', long)]
    yes: bool,
}

#[derive(Debug)]
pub struct Media {
    /// The original path to the file.
    path: PathBuf,
    /// The new generated filename.
    new_name: String,
    /// A cached version of the file extension.
    ext: &'static str,
}

impl Refine for Rename {
    type Media = Media;
    const OPENING_LINE: &'static str = "Renaming files...";
    const ENTRY_KIND: EntryKind = EntryKind::Both;

    fn refine(self, mut medias: Vec<Self::Media>) -> Result<()> {
        let kind = |p: &Path| if p.is_dir() { "/" } else { "" };

        // step: apply strip rules.
        utils::strip_filenames(
            &mut medias,
            [&self.strip_before, &self.strip_after, &self.strip_exact],
        )?;

        // step: apply replacement rules.
        for (k, v) in &self.replace {
            let re = Regex::new(&format!("(?i){k}"))
                .with_context(|| format!("compiling regex: {k:?}"))?;
            medias.iter_mut().for_each(|m| {
                if let Cow::Owned(s) = re.replace_all(&m.new_name, v) {
                    m.new_name = s;
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
            .try_for_each(|m| write!(m.new_name, ".{}", m.ext))?;

        // step: disallow changes in directories where clashes are detected.
        medias.sort_unstable_by(|m, n| m.path.cmp(&n.path));
        medias
            .chunk_by_mut(|m, n| m.path.parent() == n.path.parent())
            .filter(|_| utils::is_running())
            .for_each(|g| {
                let path = g[0].path.parent().unwrap_or(Path::new("/")).to_owned();
                let mut clashes = HashMap::with_capacity(g.len());
                g.iter().for_each(|m| {
                    clashes
                        .entry(&m.new_name)
                        .or_insert_with(Vec::new)
                        .push(&m.path)
                });
                clashes.retain(|_, v| v.len() > 1);
                if !clashes.is_empty() {
                    eprintln!("warning: names clash in: {}/", path.display());
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
                    match self.clashes {
                        false => g.iter_mut().for_each(|m| m.new_name.clear()),
                        true => {
                            let keys = clashes.iter().map(|&(k, _)| k.clone()).collect::<Vec<_>>();
                            g.iter_mut()
                                .filter(|m| keys.contains(&m.new_name))
                                .for_each(|m| m.new_name.clear());
                        }
                    }
                }
            });

        utils::user_aborted()?;

        // step: settle changes.
        let mut changes = medias
            .into_iter()
            .filter(|m| !m.new_name.is_empty()) // new clash detection.
            .filter(|m| m.new_name != m.path.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>();

        // step: display the results by parent directory.
        changes.sort_unstable_by(|m, n| {
            (Reverse(m.path.components().count()), &m.path)
                .cmp(&(Reverse(n.path.components().count()), &n.path))
        });
        changes
            .chunk_by(|m, n| m.path.parent() == n.path.parent())
            .for_each(|g| {
                println!("{}/:", g[0].path.parent().unwrap().display());
                g.iter().for_each(|m| {
                    println!(
                        "  {}{} --> {}{}",
                        m.path.file_name().unwrap().to_str().unwrap(),
                        kind(&m.path),
                        m.new_name,
                        kind(&m.path),
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
        if !self.yes {
            utils::prompt_yes_no("apply changes?")?;
        }
        utils::rename_move_consuming(&mut changes);

        match changes.is_empty() {
            true => println!("done"),
            false => println!("found {} errors", changes.len()),
        }
        Ok(())
    }
}

impl utils::NewName for Media {
    fn new_name(&mut self) -> &mut String {
        &mut self.new_name
    }
}

impl utils::OriginalPath for Media {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl utils::NewPath for Media {
    fn new_path(&self) -> PathBuf {
        self.path.with_file_name(&self.new_name)
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let (name, ext) = utils::filename_parts(&path)?;
        Ok(Media {
            new_name: name.trim().to_owned(),
            ext: utils::intern(ext),
            path,
        })
    }
}
