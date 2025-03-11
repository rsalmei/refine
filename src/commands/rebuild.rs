use crate::commands::Refine;
use crate::entries::{Entry, EntrySupport, Warnings};
use crate::media::FileOps;
use crate::naming::NamingRules;
use crate::utils;
use crate::{impl_new_name, impl_new_name_mut, impl_original_path};
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
    naming_rules: NamingRules,
    /// Disable smart matching, so "foo bar.mp4", "FooBar.mp4" and "foo__bar.mp4" are different.
    #[arg(short = 's', long)]
    simple: bool,
    /// Force to overwrite filenames (use the Global options to filter files).
    #[arg(short = 'f', long, value_name = "STR", conflicts_with_all = ["strip_before", "strip_after", "strip_exact", "replace", "simple", "partial"], value_parser = NonEmptyStringValueParser::new())]
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
    smart_match: Option<String>,
    /// The sequence number, which will be kept in partial mode and disambiguate `created` in all modes.
    seq: Option<usize>,
    /// A cached version of the file extension.
    ext: &'static str,
    /// The creation time of the file.
    created: SystemTime,
}

static CASE_FN: OnceLock<fn(&str) -> String> = OnceLock::new();

impl Refine for Rebuild {
    type Media = Media;
    const OPENING_LINE: &'static str = "Rebuilding files...";
    const SUPPORT: EntrySupport = EntrySupport::Files;

    fn tweak(&mut self, warnings: &Warnings) {
        let f = match self.case {
            false => str::to_lowercase,
            true => str::to_owned,
        };
        CASE_FN.set(f).unwrap();

        if warnings.missing && !self.partial && self.force.is_none() {
            self.partial = true;
            eprintln!("Enabling partial mode due to missing directories.\n");
        }
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        let total_files = medias.len();

        // step: apply naming rules.
        let warnings = self.naming_rules.apply(&mut medias)?;

        // step: extract and strip sequence numbers.
        medias.iter_mut().for_each(|m| {
            let (name, seq, _) = m.entry.collection_parts();
            m.seq = seq;
            m.new_name.truncate(name.len()); // sequence numbers are always at the end.
        });

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
                    m.smart_match = Some(x);
                }
            });
        }

        // helper closures to pick names and sequences.
        let name_idx = if self.simple || self.force.is_some() {
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
            // smart matching which chooses the longest name, i.e. the one with the most space and _ characters.
            |g: &[Media]| {
                g.iter()
                    .enumerate()
                    .max_by_key(|&(_, m)| m.new_name.len()) // find the longer one.
                    .unwrap()
                    .0
            }
        };
        let p_seq = match self.partial {
            true => |m: &Media| m.seq,    // retain previous sequences.
            false => |_: &Media| Some(1), // completely ignore previous sequences.
        };
        let s_seq = |m: &Media| p_seq(m).unwrap_or(usize::MAX); // files with a sequence first, no sequence last.

        // step: generate new names.
        medias.sort_unstable_by(|m, n| {
            // unfortunately, some file systems have low resolution creation time, HFS+ for example, so seq is used to disambiguate `created`.
            (m.group(), s_seq(m), m.created, m.seq).cmp(&(n.group(), s_seq(n), n.created, n.seq))
        });
        let mut total_names = 0;
        medias
            .chunk_by_mut(|m, n| m.group() == n.group())
            .for_each(|g| {
                total_names += 1;
                let base = g[name_idx(g)].new_name.clone(); // must be cloned because `g` will be modified below.
                let mut seq = p_seq(&g[0]).unwrap_or(1); // the minimum found for this group will be the first.
                g.iter_mut().for_each(|m| {
                    let (dot, ext) = match m.ext.is_empty() {
                        true => ("", ""),
                        false => (".", m.ext),
                    };
                    m.new_name = format!("{base}-{seq}{dot}{ext}");
                    seq += 1; // fixes gaps even in partial mode.
                });
            });

        utils::aborted()?;

        // step: settle changes, and display the results.
        medias.retain(|m| m.new_name != m.entry.file_name().unwrap().to_str().unwrap());
        medias
            .iter()
            .for_each(|m| println!("{} --> {}", m.entry.display(), m.new_name));

        // step: display receipt summary.
        if !medias.is_empty() || warnings > 0 {
            println!();
        }
        println!("total files: {total_files} ({total_names} names)");
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
                Ok(()) => m.entry.set_file_name(&temp),
                Err(err) => eprintln!("error: {err:?}: {:?} --> {temp:?}", m.entry),
            }
        });
        medias.rename_move_consuming();

        match medias.is_empty() {
            true => println!("done"),
            false => println!("still {} errors, giving up", medias.len()),
        }
        Ok(())
    }
}

impl_new_name!(Media);
impl_new_name_mut!(Media);
impl_original_path!(Media);

impl Media {
    /// The group name will either be the smart match or the new name.
    fn group(&self) -> &str {
        self.smart_match.as_deref().unwrap_or(&self.new_name)
    }
}

impl TryFrom<Entry> for Media {
    type Error = anyhow::Error;

    fn try_from(entry: Entry) -> Result<Self> {
        let (name, ext) = entry.filename_parts();
        Ok(Media {
            new_name: CASE_FN.get().unwrap()(name.trim()),
            ext: utils::intern(ext),
            created: entry.metadata()?.created()?,
            seq: None, // can't be set here, because naming rules must run before it.
            smart_match: None,
            entry,
        })
    }
}
