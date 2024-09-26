use crate::commands::Refine;
use crate::entries::EntryKind;
use crate::utils::{self, Sequence};
use anyhow::Result;
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
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
    #[arg(short = 'f', long, value_name = "STR", conflicts_with_all = ["strip_before", "strip_after", "strip_exact", "no_smart_detect"], value_parser = NonEmptyStringValueParser::new())]
    force: Option<String>,
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

    fn refine(self, mut medias: Vec<Self::Media>) -> Result<()> {
        let total = medias.len();
        let warnings = if let Some(force) = &self.force {
            medias.iter_mut().for_each(|m| {
                m.new_name.clone_from(force);
            });
            0
        } else {
            // step: strip sequence numbers.
            medias.iter_mut().for_each(|m| {
                if let Some(Sequence { len, .. }) = utils::extract_sequence(&m.new_name) {
                    m.new_name.truncate(m.new_name.len() - len); // sequence numbers are always at the end.
                }
            });

            // step: apply strip rules.
            utils::strip_filenames(
                &mut medias,
                [&self.strip_before, &self.strip_after, &self.strip_exact],
            )?;

            utils::user_aborted()?;

            // step: remove medias where the rules cleared the name.
            let warnings = utils::remove_cleared(&mut medias);

            // step: smart detect.
            if !self.no_smart_detect {
                static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s_]+").unwrap());

                medias.iter_mut().for_each(|m| {
                    if let Cow::Owned(x) = RE.replace_all(&m.new_name, "") {
                        m.smart_group = Some(x);
                    }
                });
            }
            warnings
        };

        // step: generate new names to compute the changes.
        apply_new_names(&mut medias, self.no_smart_detect);

        utils::user_aborted()?;

        // step: settle changes, and display the results.
        let mut changes = medias
            .into_iter()
            .filter(|m| m.new_name != m.path.file_name().unwrap().to_str().unwrap()) // the list might have changed on force.
            .inspect(|m| {
                println!("{} --> {}", m.path.display(), m.new_name);
            })
            .collect::<Vec<_>>();

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
        if changes.is_empty() {
            println!("done");
            return Ok(());
        }

        // step: fix file already exists errors.
        println!("attempting to fix {} errors", changes.len());
        changes.iter_mut().for_each(|m| {
            let temp = format!("__refine+{}__", m.new_name);
            let dest = m.path.with_file_name(&temp);
            match fs::rename(&m.path, &dest) {
                Ok(()) => m.path = dest,
                Err(err) => eprintln!("error: {err:?}: {:?} --> {temp:?}", m.path),
            }
        });
        utils::rename_move_consuming(&mut changes);

        match changes.is_empty() {
            true => println!("done"),
            false => println!("still {} errors, giving up", changes.len()),
        }
        Ok(())
    }
}

fn apply_new_names(medias: &mut [Media], no_smart_detect: bool) {
    medias.sort_unstable_by(|m, n| m.group().cmp(n.group()));
    medias
        .chunk_by_mut(|m, n| m.group() == n.group())
        .for_each(|g| {
            g.sort_by_key(|m| m.created);
            let base = if no_smart_detect {
                &g[0].new_name
            } else {
                let vars = g.iter().map(|m| &m.new_name).collect::<HashSet<_>>();
                vars.iter().map(|&x| (x.len(), x)).max().unwrap().1
            };
            let base = base.replace(' ', "_");
            g.iter_mut().enumerate().for_each(|(i, m)| {
                m.new_name.clear(); // because of the force option.
                write!(m.new_name, "{base}-{}", i + 1).unwrap();
                if !m.ext.is_empty() {
                    write!(m.new_name, ".{}", m.ext).unwrap();
                }
            });
        });
}

impl utils::NewName for Media {
    fn new_name(&self) -> &str {
        &self.new_name
    }
    fn new_name_mut(&mut self) -> &mut String {
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
