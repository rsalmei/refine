use crate::commands::Refine;
use crate::entries::{Entry, TraversalMode};
use crate::medias::{FileOps, NamingSpec};
use crate::utils;
use crate::{impl_new_name, impl_new_name_mut, impl_source_entry};
use anyhow::Result;
use clap::{Args, ValueEnum};
use std::cmp::Reverse;
use std::fmt::{Display, Write};

#[derive(Debug, Args)]
pub struct Rename {
    #[command(flatten)]
    naming: NamingSpec,
    /// How to resolve clashes.
    #[arg(short = 'c', long, default_value_t = Clashes::Sequence, value_name = "STR", value_enum)]
    clashes: Clashes,
    /// Skip the confirmation prompt, useful for automation.
    #[arg(short = 'y', long)]
    yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Clashes {
    #[value(aliases = ["s", "seq"])]
    Sequence,
    #[value(aliases = ["i", "ig"])]
    Ignore,
    #[value(aliases = ["f", "ff"])]
    Forbid,
}

#[derive(Debug)]
pub struct Media {
    /// The original path to the file.
    entry: Entry,
    /// The new generated filename.
    new_name: String,
    /// A cached version of the file extension.
    ext: &'static str,
    /// Marks resolution of clashes.
    resolution: &'static str,
}

impl Refine for Rename {
    type Media = Media;
    const OPENING_LINE: &'static str = "Rename files";
    const MODE: TraversalMode = TraversalMode::DirsAndContent;

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        let total_files = medias.len();

        // step: apply naming rules.
        let mut blocked = self.naming.compile()?.apply(&mut medias);

        // step: re-include extension in the names.
        medias
            .iter_mut()
            .filter(|m| !m.ext.is_empty())
            .try_for_each(|m| write!(m.new_name, ".{}", m.ext))?;

        // step: clashes resolution.
        let mut clashes = 0;
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
                eprintln!("warning: names clash in: {}", g[0].entry.parent().unwrap());
                g.chunk_by(|m, n| m.new_name == n.new_name)
                    .filter(|g| g.len() > 1)
                    .for_each(|g| {
                        let k = &g[0].new_name;
                        let list = g
                            .iter()
                            .map(|m| m.entry.file_name())
                            .filter(|f| f != k)
                            .collect::<Vec<_>>();
                        clashes += list.len();
                        use yansi::Paint;
                        let msg = match g.len() != list.len() {
                            true => " name already exists",
                            false => " multiple names clash",
                        };
                        eprintln!(
                            "  > {} --> {k}{}",
                            list.join(", "),
                            msg.paint(yansi::Color::BrightMagenta)
                        );
                    });
                match self.clashes {
                    Clashes::Forbid => {
                        let count = g.iter().filter(|m| m.is_changed()).count();
                        blocked += count;
                        eprintln!("  ...blocked {count} changes in this folder");
                        g.iter_mut().for_each(|m| m.new_name.clear());
                    }
                    Clashes::Ignore => g
                        .chunk_by_mut(|m, n| m.new_name == n.new_name)
                        .filter(|g| g.len() > 1)
                        .for_each(|g| g.iter_mut().for_each(|m| m.new_name.clear())),
                    Clashes::Sequence => {
                        g.chunk_by_mut(|m, n| m.new_name == n.new_name)
                            .filter(|g| g.len() > 1)
                            .for_each(|g| {
                                g.iter_mut().filter(|m| m.is_changed()).zip(1..).for_each(
                                    |(m, i)| {
                                        m.new_name.truncate(m.new_name.len() - m.ext.len() - 1);
                                        write!(m.new_name, "-{i}.{}", m.ext).unwrap();
                                        m.resolution = " (added sequence number)";
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
            // requires a post-order like traversal to avoid move errors.
            // but since I couldn't find a way to do that, I just reverse the order.
            // that way, the deepest directories are processed first, before their parents.
            (Reverse(m.entry.parent()), &m.entry).cmp(&(Reverse(n.entry.parent()), &n.entry))
        });
        medias
            .chunk_by(|m, n| m.entry.parent() == n.entry.parent())
            .for_each(|g| {
                println!("{}", g[0].entry.parent().unwrap());
                use yansi::Paint;
                g.iter().for_each(|m| {
                    println!(
                        "  {} --> {}{}",
                        m.entry.display_filename(),
                        m.new_name,
                        m.resolution.paint(yansi::Color::BrightBlue)
                    )
                });
            });

        // step: display a summary receipt.
        if !medias.is_empty() || blocked > 0 {
            println!();
        }
        println!("total files: {total_files}");
        println!("  changes: {}", medias.len());
        println!("  clashes: {clashes} ({})", self.clashes);
        println!("  blocked: {blocked}");
        if medias.is_empty() {
            return Ok(());
        }

        // step: apply changes if the user agrees.
        if !self.yes {
            utils::prompt_yes_no("apply changes?")?;
        }
        FileOps::rename_move(&mut medias);

        match medias.is_empty() {
            true => println!("done"),
            false => println!("found {} errors", medias.len()),
        }
        Ok(())
    }
}

impl Display for Clashes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Clashes::Sequence => write!(f, "resolved by adding a sequence number"),
            Clashes::Ignore => write!(f, "ignored, folders processed as usual"),
            Clashes::Forbid => write!(f, "whole folders with clashes blocked"),
        }
    }
}

impl_source_entry!(Media);
impl_new_name!(Media);
impl_new_name_mut!(Media);

impl Media {
    fn is_changed(&self) -> bool {
        self.new_name != self.entry.file_name()
    }
}

impl TryFrom<&Entry> for Media {
    type Error = anyhow::Error;

    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        let (stem, ext) = entry.filename_parts();
        Ok(Media {
            new_name: stem.trim().to_owned(),
            ext: utils::intern(ext),
            entry: entry.to_owned(),
            resolution: "",
        })
    }
}
