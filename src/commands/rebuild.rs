use crate::commands::Refine;
use crate::entries::{Entry, InputInfo, TraversalMode};
use crate::medias::{FileOps, NamingSpec};
use crate::utils::{self, PromptError};
use crate::{impl_new_name, impl_new_name_mut, impl_source_entry};
use anyhow::Result;
use clap::Args;
use clap::builder::NonEmptyStringValueParser;
use regex::Regex;
use std::borrow::Cow;
use std::fs;
use std::sync::{LazyLock, OnceLock};
use std::time::SystemTime;

#[derive(Debug, Args)]
pub struct Rebuild {
    #[command(flatten)]
    naming: NamingSpec,
    /// Disable smart matching, so "foo bar.mp4", "FooBar.mp4" and "foo__bar.mp4" are different.
    #[arg(short = 's', long)]
    simple: bool,
    /// Force to overwrite filenames (use the Global options to filter files).
    #[arg(short = 'f', long, value_name = "STR", conflicts_with_all = ["strip_before", "strip_after", "strip_exact", "replace", "throw", "simple", "partial"], value_parser = NonEmptyStringValueParser::new())]
    force: Option<String>,
    /// Assume not all directories are available, which retains current sequences (but fixes gaps).
    #[arg(short = 'p', long)]
    partial: bool,
    /// Keep the original case of filenames, otherwise they are lowercased.
    #[arg(short = 'c', long)]
    case: bool,
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
    /// The resulting smart match (if enabled and new_name has spaces or _).
    group_name: Option<String>,
    /// The sequence number, which will be kept in partial mode and disambiguate `created` in all modes.
    seq: Option<usize>,
    /// A comment for the file.
    comment: String,
    /// A cached version of the file extension.
    ext: &'static str,
    /// The creation time of the file.
    created: SystemTime,
}

static CASE_FN: OnceLock<fn(&str) -> String> = OnceLock::new();

impl Refine for Rebuild {
    type Media = Media;
    const OPENING_LINE: &'static str = "Rebuild collection filenames";
    const MODE: TraversalMode = TraversalMode::Files;

    fn tweak(&mut self, input: &InputInfo) {
        let f = match self.case {
            false => str::to_lowercase,
            true => str::to_owned,
        };
        CASE_FN.set(f).unwrap();

        if input.has_invalid && !self.partial && self.force.is_none() {
            self.partial = true;
            eprintln!("Enabling partial mode due to missing directories.\n");
        }
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        let total_files = medias.len();

        // detect if migration is needed.
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\w+)-(\d+)$").unwrap());
        if medias
            .iter()
            .any(|m| m.seq.is_none() && RE.is_match(m.entry.filename_parts().0))
        {
            eprintln!("warning: detected old-style filenames.");
            match utils::prompt_yes_no(r#"migrate to new style "name~9"?"#) {
                Ok(()) => {
                    medias.iter_mut().for_each(|m| {
                        if let Some(caps) = RE.captures(m.entry.filename_parts().0) {
                            m.new_name.truncate(caps[1].len()); // truncate to the actual name length.
                            m.seq = caps[2].parse().ok(); // find the actual sequence number.
                        }
                    });
                }
                Err(PromptError::No) => {
                    eprintln!("filenames might be inconsistent.");
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        // step: apply naming rules.
        let blocked = self.naming.compile()?.apply(&mut medias);

        // step: reset names if forcing a new one.
        if let Some(force) = &self.force {
            medias.iter_mut().for_each(|m| {
                m.new_name.clone_from(force);
            });
        }

        // step: prepare smart matching groups.
        if !self.simple {
            static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s_]+").unwrap());

            medias.iter_mut().for_each(|m| {
                if let Cow::Owned(x) = RE.replace_all(&m.new_name, "") {
                    m.group_name = Some(x);
                }
            });
        }

        // step: sort medias according to partial or full mode.
        let seq = match self.partial {
            true => |m: &Media| m.seq.unwrap_or(usize::MAX), // no sequence goes to the end in partial mode.
            false => |_: &Media| 0,                          // ignore sequences in full mode.
        };
        medias.sort_unstable_by(|m, n| {
            // unfortunately, some file systems have low-resolution creation time, HFS+ for example,
            // so m.seq is used to disambiguate `created`, which seems to repeat a lot sometimes.
            (m.group(), seq(m), m.created, m.seq).cmp(&(n.group(), seq(n), n.created, n.seq))
        });

        // step: generate new names.
        let name_idx = if self.simple {
            |_g: &[Media]| 0 // all the names are exactly the same within a group.
        } else if self.case {
            // smart matching which chooses the name with the most uppercase characters.
            |g: &[Media]| {
                g.iter()
                    .enumerate()
                    .max_by_key(|&(_, m)| m.new_name.chars().filter(|c| c.is_uppercase()).count())
                    .unwrap()
                    .0
            }
        } else {
            // smart matching which chooses the longest name, i.e., the one with the most space and _ characters.
            |g: &[Media]| {
                g.iter()
                    .enumerate()
                    .max_by_key(|&(_, m)| m.new_name.len()) // find the longer one.
                    .unwrap()
                    .0
            }
        };
        let seq_gen = match self.partial {
            true => |m: &Media, last_seq: usize| m.seq.unwrap_or_else(|| last_seq + 1),
            false => |_: &Media, last_seq: usize| last_seq + 1,
        };
        let mut unique_names = 0;
        medias
            .chunk_by_mut(|m, n| m.group() == n.group())
            .for_each(|g| {
                unique_names += 1;
                let base = std::mem::take(&mut g[name_idx(g)].new_name); // must be taken because `g` will be modified below.
                let mut seq = 0; // keep track of the last sequence number used.
                g.iter_mut().for_each(|m| {
                    seq = seq_gen(m, seq);
                    let dot = if m.ext.is_empty() { "" } else { "." };
                    m.new_name = format!("{base}~{seq}{}{dot}{}", m.comment, m.ext);
                });
            });

        utils::aborted()?;

        // step: settle changes, and display the results.
        medias.retain(|m| m.new_name != m.entry.file_name());
        medias
            .iter()
            .for_each(|m| println!("{} --> {}", m.entry, m.new_name));

        // step: display a summary receipt.
        if !medias.is_empty() || blocked > 0 {
            println!();
        }
        println!("total files: {total_files} ({unique_names} unique names)");
        println!("  changes: {}", medias.len());
        println!("  blocked: {blocked}");
        if medias.is_empty() {
            return Ok(());
        }

        // step: apply changes if the user agrees.
        if !self.yes {
            utils::prompt_yes_no("apply changes?")?;
        }
        FileOps::rename_move(&mut medias);
        if medias.is_empty() {
            println!("done");
            return Ok(());
        }

        // step: fix file already exists errors.
        println!("attempting to fix {} errors", medias.len());
        medias.iter_mut().for_each(|m| {
            let temp = format!("__refine+{}__", m.new_name);
            let dest = m.entry.with_file_name(&temp);
            match fs::rename(&m.entry, &dest) {
                Ok(()) => m.entry = dest,
                Err(err) => eprintln!("error: {err}: {} --> {temp:?}", m.entry),
            }
        });
        FileOps::rename_move(&mut medias);

        match medias.is_empty() {
            true => println!("done"),
            false => println!("still {} errors, giving up", medias.len()),
        }
        Ok(())
    }
}

impl_source_entry!(Media);
impl_new_name!(Media);
impl_new_name_mut!(Media);

impl Media {
    /// The group name will either be the smart match or the new name.
    fn group(&self) -> &str {
        self.group_name.as_deref().unwrap_or(&self.new_name)
    }
}

impl TryFrom<Entry> for Media {
    type Error = (Entry, anyhow::Error);

    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
        let (name, _, seq, comment, ext) = entry.collection_parts();
        let created = entry.metadata().map_or(None, |m| m.created().ok());
        Ok(Media {
            new_name: CASE_FN.get().unwrap()(name.trim()),
            group_name: None,
            seq,
            comment: comment.to_string(),
            ext: utils::intern(ext),
            created: created.unwrap_or(SystemTime::now()),
            entry,
        })
    }
}
