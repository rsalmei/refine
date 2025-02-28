use super::{Entry, EntryKinds, Refine};
use crate::media::FileOps;
use crate::naming::NamingRules;
use crate::utils;
use crate::{impl_new_name, impl_new_name_mut, impl_original_path};
use anyhow::Result;
use clap::{Args, ValueEnum};
use std::cmp::Reverse;
use std::fmt::Write;

#[derive(Debug, Args)]
pub struct Rename {
    #[command(flatten)]
    naming_rules: NamingRules,
    /// How to resolve clashes.
    #[arg(short = 'c', long, default_value_t = Clashes::Forbid, value_name = "STR", value_enum)]
    clashes: Clashes,
    /// Skip the confirmation prompt, useful for automation.
    #[arg(short = 'y', long)]
    yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Clashes {
    #[value(aliases = ["f", "fb"])]
    Forbid,
    #[value(aliases = ["i", "ig"])]
    Ignore,
    #[value(aliases = ["s", "sq", "seq", "ns"])]
    NameSequence,
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
    const REQUIRE: EntryKinds = EntryKinds::DirAndFiles;

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
        medias.sort_unstable_by(|m, n| {
            (m.entry.parent(), &m.new_name).cmp(&(n.entry.parent(), &n.new_name))
        });
        medias
            .chunk_by_mut(|m, n| m.entry.parent() == n.entry.parent()) // only by parent.
            .filter(|_| utils::is_running())
            .filter(|g| {
                g.chunk_by(|m, n| m.new_name == n.new_name)
                    .any(|g| g.len() > 1) // this should be way faster than using a hashmap as before.
            })
            .for_each(|g| {
                eprintln!(
                    "warning: names clash in: {}/",
                    g[0].entry.parent().unwrap().display()
                );
                g.chunk_by(|m, n| m.new_name == n.new_name)
                    .filter(|g| g.len() > 1)
                    .for_each(|g| {
                        let k = &g[0].new_name;
                        let list = g
                            .iter()
                            .map(|m| m.filename())
                            .filter(|f| f != k)
                            .collect::<Vec<_>>();
                        warnings += list.len();
                        let exists = if g.len() != list.len() { " exists" } else { "" };
                        eprintln!("  > {} --> {k}{exists}", list.join(", "));
                    });
                match self.clashes {
                    Clashes::Forbid => g.iter_mut().for_each(|m| m.new_name.clear()),
                    Clashes::Ignore => g
                        .chunk_by_mut(|m, n| m.new_name == n.new_name)
                        .filter(|g| g.len() > 1)
                        .for_each(|g| g.iter_mut().for_each(|m| m.new_name.clear())),
                    Clashes::NameSequence => {
                        g.chunk_by_mut(|m, n| m.new_name == n.new_name)
                            .filter(|g| g.len() > 1)
                            .for_each(|g| {
                                g.iter_mut().filter(|m| m.is_changed()).zip(1..).for_each(
                                    |(m, i)| {
                                        m.new_name.truncate(m.new_name.len() - m.ext.len() - 1);
                                        write!(m.new_name, "-{i}.{}", m.ext).unwrap();
                                    },
                                )
                            })
                    }
                }
            });

        utils::aborted()?;

        // step: settle changes.
        medias.retain(|m| !m.new_name.is_empty() && m.is_changed());

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

impl Media {
    fn is_changed(&self) -> bool {
        self.new_name != self.filename()
    }
    fn filename(&self) -> &str {
        self.entry.file_name().unwrap().to_str().unwrap()
    }
}

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
