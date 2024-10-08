use crate::commands::Refine;
use crate::entries::{Entries, EntryKind};
use crate::utils::{self, Sequence};
use crate::{impl_new_name, impl_original_path};
use anyhow::Result;
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::SystemTime;

#[derive(Debug, Args)]
pub struct Rebuild {
    /// Remove from the start of the filename to this str; blanks are automatically removed.
    #[arg(short = 'b', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_before: Vec<String>,
    /// Remove from this str to the end of the filename; blanks are automatically removed.
    #[arg(short = 'a', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_after: Vec<String>,
    /// Remove all occurrences of this str in the filename; blanks are automatically removed.
    #[arg(short = 'e', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_exact: Vec<String>,
    /// Detect and fix similar filenames (e.g. "foo bar.mp4" and "foo__bar.mp4").
    #[arg(short = 's', long)]
    no_smart_detect: bool,
    /// Easily overwrite filenames (use the Global options to filter them).
    #[arg(short = 'f', long, value_name = "STR", conflicts_with_all = ["strip_before", "strip_after", "strip_exact", "no_smart_detect", "partial"], value_parser = NonEmptyStringValueParser::new())]
    force: Option<String>,
    /// Assume some paths are not available, so only touch files actually modified by the given rules.
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
    /// The smart group (if enabled and wname has spaces or _).
    smart_group: Option<String>,
    /// A cached version of the file extension.
    ext: &'static str,
    /// The creation time of the file.
    created: SystemTime,
}

impl Refine for Rebuild {
    type Media = Media;
    const OPENING_LINE: &'static str = "Rebuilding files...";
    const ENTRY_KIND: EntryKind = EntryKind::File;

    fn adjust(&mut self, entries: &Entries) {
        if entries.missing && !self.partial && self.force.is_none() {
            self.partial = true;
            eprintln!("warning: one or more paths are not available => enabling partial mode\n");
        }
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        let (total, mut warnings) = (medias.len(), 0);
        if let Some(force) = &self.force {
            medias.iter_mut().for_each(|m| {
                m.new_name.clone_from(force);
            });
        } else {
            // step: strip sequence numbers.
            medias.iter_mut().for_each(|m| {
                m.new_name.truncate(utils::real_length(&m.new_name)); // sequence numbers are always at the end.
            });

            // step: apply strip rules.
            utils::strip_filenames(
                &mut medias,
                [&self.strip_before, &self.strip_after, &self.strip_exact],
            )?;

            // step: remove medias where the rules cleared the name.
            warnings += utils::remove_cleared(&mut medias);

            // step: smart detect.
            if !self.no_smart_detect {
                static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s_]+").unwrap());

                medias.iter_mut().for_each(|m| {
                    if let Cow::Owned(x) = RE.replace_all(&m.new_name, "") {
                        m.smart_group = Some(x);
                    }
                });
            }
        };

        // step: generate new names before computing the changes.
        let name_picker = if self.no_smart_detect || self.force.is_some() {
            |g: &[Media]| g[0].new_name.to_owned() // must return owned value because new_name mustn't be borrowed to be modified.
        } else {
            |g: &[Media]| {
                let nn = g.iter().map(|m| &m.new_name).collect::<HashSet<_>>();
                nn.iter().map(|&x| (x.len(), x)).max().unwrap().1.to_owned()
            }
        };
        medias.sort_unstable_by(|m, n| m.group().cmp(n.group()));
        medias
            .chunk_by_mut(|m, n| m.group() == n.group())
            .for_each(|g| {
                g.sort_by_key(|m| m.created);
                let base = name_picker(g); // this used to have a .replace(' ', "_")... I don't remember why.
                g.iter_mut().enumerate().for_each(|(i, m)| {
                    m.new_name.clear(); // because of the force and smart options.
                    write!(m.new_name, "{base}-{}", i + 1).unwrap();
                    if !m.ext.is_empty() {
                        write!(m.new_name, ".{}", m.ext).unwrap();
                    }
                });
            });

        utils::user_aborted()?;

        // step: settle changes, and display the results.
        medias.retain(|m| m.new_name != m.path.file_name().unwrap().to_str().unwrap()); // the list might have changed on force.
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
            smart_group: None,
            path,
        })
    }
}
