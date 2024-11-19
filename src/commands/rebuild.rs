use crate::commands::Refine;
use crate::entries::{Entries, EntrySet};
use crate::utils::{self, NamingRules, Sequence};
use crate::{impl_new_name, impl_new_name_mut, impl_original_path};
use anyhow::Result;
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::SystemTime;

#[derive(Debug, Args)]
pub struct Rebuild {
    #[command(flatten)]
    naming_rules: NamingRules,
    /// Disable smart detection of similar filenames (e.g. "foo bar.mp4", "FooBar.mp4" and "foo__bar.mp4").
    #[arg(short = 's', long)]
    no_smart_detect: bool,
    /// Force to overwrite filenames (use the Global options to filter files).
    #[arg(short = 'f', long, value_name = "STR", conflicts_with_all = ["strip_before", "strip_after", "strip_exact", "replace", "no_smart_detect", "partial"], value_parser = NonEmptyStringValueParser::new())]
    force: Option<String>,
    /// Assume not all paths are available, so only touch files actually modified by the given rules.
    #[arg(short = 'p', long)]
    partial: bool,
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
    /// The smart group (if enabled and new_name has spaces or _).
    smart_group: Option<String>,
    /// The current sequence number, which will be kept in partial mode.
    seq: Option<usize>,
    /// A cached version of the file extension.
    ext: &'static str,
    /// The creation time of the file.
    created: SystemTime,
}

impl Refine for Rebuild {
    type Media = Media;
    const OPENING_LINE: &'static str = "Rebuilding files...";
    const ENTRY_SET: EntrySet = EntrySet::Files;

    fn adjust(&mut self, entries: &Entries) {
        if entries.missing_dirs() && !self.partial && self.force.is_none() {
            self.partial = true;
            eprintln!("warning: one or more paths are not available => enabling partial mode\n");
        }
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        let (total, mut warnings) = (medias.len(), 0);

        // step: apply naming rules.
        warnings += self.naming_rules.apply(&mut medias)?;

        if let Some(force) = &self.force {
            medias.iter_mut().for_each(|m| {
                m.new_name.clone_from(force);
            });
        } else if self.partial {
            // step: extract sequence numbers so they are reused.
            medias.iter_mut().for_each(|m| {
                let seq = Sequence::from(&m.new_name);
                m.seq = Some(seq.num); // only set in partial mode, meaning this must be kept.
                m.new_name.truncate(seq.actual_len); // sequence numbers are always at the end.
            });
        } else {
            // step: strip sequence numbers.
            medias.iter_mut().for_each(|m| {
                m.new_name.truncate(Sequence::from(&m.new_name).actual_len); // sequence numbers are always at the end.
            });
        };

        // step: smart detect on full media set (including unchanged files in partial mode).
        if !self.no_smart_detect {
            static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s_]+").unwrap());

            medias.iter_mut().for_each(|m| {
                if let Cow::Owned(x) = RE.replace_all(&m.new_name, "") {
                    m.smart_group = Some(x);
                }
            });
        }

        // helper closure to pick names, varies according to the current mode.
        let name_idx = if self.no_smart_detect || self.force.is_some() {
            |_g: &[Media]| 0 // all the names are exactly the same.
        } else {
            |g: &[Media]| {
                g.iter()
                    .enumerate()
                    .max_by_key(|&(_, m)| m.new_name.len()) // find the longer one.
                    .unwrap()
                    .0
            }
        };

        // step: generate new names.
        let sort_seq = |m: &Media| m.seq.unwrap_or(usize::MAX);
        medias.sort_unstable_by(|m, n| {
            (m.group(), sort_seq(m), m.created).cmp(&(n.group(), sort_seq(n), m.created))
        });
        medias
            .chunk_by_mut(|m, n| m.group() == n.group())
            .for_each(|g| {
                let base = g[name_idx(g)].new_name.to_owned(); // must be owned because `g` will be modified below.
                let mut seq = g[0].seq.unwrap_or(1); // the minimum found for this group will be the first.
                g.iter_mut().for_each(|m| {
                    let (dot, ext) = if m.ext.is_empty() {
                        ("", "")
                    } else {
                        (".", m.ext)
                    };
                    m.new_name = format!("{base}-{seq}{dot}{ext}");
                    seq += 1;
                });
            });

        utils::user_aborted()?;

        // step: settle changes, and display the results.
        medias.retain(|m| m.new_name != m.path.file_name().unwrap().to_str().unwrap());
        medias
            .iter()
            .for_each(|m| println!("{} --> {}", m.path.display(), m.new_name));

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
        utils::rename_move_consuming(&mut medias);
        if medias.is_empty() {
            println!("done");
            return Ok(());
        }

        // step: fix file already exists errors.
        println!("attempting to fix {} errors", medias.len());
        medias.iter_mut().for_each(|m| {
            let temp = format!("__refine+{}__", m.new_name);
            let dest = m.path.with_file_name(&temp);
            match fs::rename(&m.path, &dest) {
                Ok(()) => m.path = dest,
                Err(err) => eprintln!("error: {err:?}: {:?} --> {temp:?}", m.path),
            }
        });
        utils::rename_move_consuming(&mut medias);

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
    fn group(&self) -> &str {
        self.smart_group.as_deref().unwrap_or(&self.new_name)
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let (name, ext) = utils::filename_parts(&path)?;
        Ok(Media {
            new_name: name.trim().to_lowercase(),
            ext: utils::intern(ext),
            created: fs::metadata(&path)?.created()?,
            seq: None,
            smart_group: None,
            path,
        })
    }
}
