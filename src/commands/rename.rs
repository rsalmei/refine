use super::{Entry, EntryKinds, Refine};
use crate::naming::NamingRules;
use crate::utils::{self, FileOps};
use crate::{impl_new_name, impl_new_name_mut, impl_original_path};
use anyhow::Result;
use clap::Args;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

#[derive(Debug, Args)]
pub struct Rename {
    #[command(flatten)]
    naming_rules: NamingRules,
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
    entry: Entry,
    /// The new generated filename.
    new_name: String,
    /// A cached version of the file extension.
    ext: &'static str,
}

impl Refine for Rename {
    type Media = Media;
    const OPENING_LINE: &'static str = "Renaming files...";
    const REQUIRE: EntryKinds = EntryKinds::Both;

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        // step: apply naming rules.
        let total = medias.len();
        let mut warnings = self.naming_rules.apply(&mut medias)?;

        // step: re-include extension in the names.
        medias
            .iter_mut()
            .filter(|m| !m.ext.is_empty())
            .try_for_each(|m| write!(m.new_name, ".{}", m.ext))?;

        // step: disallow changes in directories where clashes are detected.
        medias.sort_unstable_by(|m, n| m.entry.cmp(&n.entry));
        medias
            .chunk_by_mut(|m, n| m.entry.parent() == n.entry.parent())
            .filter(|_| utils::is_running())
            .for_each(|g| {
                let path = g[0].entry.parent().unwrap_or(Path::new("/")).to_owned();
                let mut clashes = HashMap::with_capacity(g.len());
                g.iter().for_each(|m| {
                    clashes
                        .entry(&m.new_name)
                        .or_insert_with(Vec::new)
                        .push(&m.entry)
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

        utils::aborted()?;

        // step: settle changes.
        medias.retain(|m| {
            !m.new_name.is_empty() // new clash detection.
            && m.new_name != m.entry.file_name().unwrap().to_str().unwrap()
        });

        // step: display the results by parent directory.
        medias.sort_unstable_by(|m, n| {
            (Reverse(m.entry.components().count()), &m.entry)
                .cmp(&(Reverse(n.entry.components().count()), &n.entry))
        });
        medias
            .chunk_by(|m, n| m.entry.parent() == n.entry.parent())
            .for_each(|g| {
                println!("{}/:", g[0].entry.parent().unwrap().display());
                g.iter().for_each(|m| {
                    println!(
                        "  {} --> {}{}",
                        m.entry.display_filename(),
                        m.new_name,
                        m.entry.kind(),
                    )
                });
            });

        // step: display receipt summary.
        if !medias.is_empty() || warnings > 0 {
            println!();
        }
        println!("total files: {total}");
        println!("  changes: {}", medias.len());
        println!("  warnings: {warnings}");
        if medias.is_empty() {
            return Ok(());
        }

        // step: apply changes, if the user agrees.
        if !self.yes {
            utils::prompt_yes_no("apply changes?")?;
        }
        medias.rename_move_consuming();

        match medias.is_empty() {
            true => println!("done"),
            false => println!("found {} errors", medias.len()),
        }
        Ok(())
    }
}

impl_new_name!(Media);
impl_new_name_mut!(Media);
impl_original_path!(Media);

impl TryFrom<Entry> for Media {
    type Error = anyhow::Error;

    fn try_from(entry: Entry) -> Result<Self> {
        let (name, ext) = entry.filename_parts();
        Ok(Media {
            new_name: name.trim().to_owned(),
            ext: utils::intern(ext),
            entry,
        })
    }
}
