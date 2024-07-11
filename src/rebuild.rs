use crate::utils::{self, StripPos};
use anyhow::Result;
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

#[derive(Debug, Args)]
pub struct Rebuild {
    /// Remove from the start of the filename to this str; blanks are automatically removed.
    #[arg(short = 'b', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub strip_before: Vec<String>,
    /// Remove from this str to the end of the filename; blanks are automatically removed.
    #[arg(short = 'a', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub strip_after: Vec<String>,
    /// Remove all occurrences of this str in the filename; blanks are automatically removed.
    #[arg(short = 'e', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    pub strip_exact: Vec<String>,
    /// Detect and fix similar filenames (e.g. "foo bar.mp4" and "foo__bar.mp4").
    #[arg(short = 's', long)]
    pub no_smart_detect: bool,
    /// Easily set filenames for new files. BEWARE: use only on already organized collections.
    #[arg(short, long, value_name = "STR", value_parser = NonEmptyStringValueParser::new())]
    pub force: Option<String>,
    /// Skip the confirmation prompt, useful for automation.
    #[arg(short, long)]
    pub yes: bool,
}

fn opt() -> &'static Rebuild {
    match &super::args().cmd {
        super::Command::Rebuild(opt) => opt,
        _ => unreachable!(),
    }
}

#[derive(Debug)]
pub struct Media {
    /// The original path to the file.
    path: PathBuf,
    /// The working copy of the name, where the rules are applied.
    wname: String,
    /// The smart group (if enabled and wname has spaces or _).
    smart_group: Option<String>,
    /// The final name, after the rules and the sequence have been applied.
    new_name: String,
    /// A cached version of the file extension.
    ext: &'static str,
    /// The creation time of the file.
    ts: SystemTime,
}

pub fn run(mut medias: Vec<Media>) -> Result<()> {
    println!("=> Rebuilding files...\n");

    // step: strip sequence numbers.
    medias.iter_mut().for_each(|m| {
        let name = utils::strip_sequence(&m.wname);
        if name != m.wname {
            m.wname.truncate(name.len()); // sequence numbers are at the end of the filename.
        }
    });

    // step: apply strip rules.
    utils::strip_names(&mut medias, StripPos::Before, &opt().strip_before)?;
    utils::strip_names(&mut medias, StripPos::After, &opt().strip_after)?;
    utils::strip_names(&mut medias, StripPos::Exact, &opt().strip_exact)?;

    // step: force names.
    if let Some(force) = &opt().force {
        medias
            .iter_mut()
            .filter(|m| m.wname.is_empty())
            .for_each(|m| {
                m.wname.clone_from(force);
            })
    }

    utils::user_aborted()?;

    // step: remove medias where the rules cleared the name.
    let total = medias.len();
    let warnings = utils::remove_cleared(&mut medias);

    // step: smart detect.
    if !opt().no_smart_detect {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r"[\s_]+").unwrap());

        medias.iter_mut().for_each(|m| {
            if let Cow::Owned(x) = re.replace_all(&m.wname, "") {
                m.smart_group = Some(x);
            }
        });
    }

    // step: generate new names to compute the changes.
    apply_new_names(&mut medias);

    // step: if forced, apply it only to the effective changes and regenerate new names.
    if let Some(force) = &opt().force {
        medias
            .iter_mut()
            .filter(|m| m.new_name != m.path.file_name().unwrap().to_str().unwrap())
            .for_each(|m| {
                m.wname.clone_from(force);
                m.smart_group = None;
            });
        apply_new_names(&mut medias);
    }

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
    if !opt().yes {
        utils::prompt_yes_no("apply changes?")?;
    }
    utils::rename_consuming(&mut changes);
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
    utils::rename_consuming(&mut changes);
    match changes.is_empty() {
        true => println!("done"),
        false => println!("still {} errors, giving up", changes.len()),
    }

    Ok(())
}

fn apply_new_names(medias: &mut [Media]) {
    medias.sort_unstable_by(|m, n| m.group().cmp(n.group()));
    medias
        .chunk_by_mut(|m, n| m.group() == n.group())
        .for_each(|g| {
            g.sort_by_key(|m| m.ts);
            let base = match opt().no_smart_detect {
                false => {
                    let vars = g.iter().map(|m| &m.wname).collect::<HashSet<_>>();
                    vars.iter().map(|&x| (x.len(), x)).max().unwrap().1
                }
                true => &g[0].wname,
            };
            let base = match base.contains(' ') {
                true => base.replace(' ', "_"),
                false => base.to_owned(), // needed because g is borrowed, and I need to mutate it below.
            };
            g.iter_mut().enumerate().for_each(|(i, m)| {
                m.new_name.clear(); // because of the force option.
                write!(m.new_name, "{base}-{}", i + 1).unwrap();
                if !m.ext.is_empty() {
                    write!(m.new_name, ".{}", m.ext).unwrap();
                }
            });
        });
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
        &self.new_name
    }
}

impl Media {
    fn group(&self) -> &str {
        self.smart_group.as_deref().unwrap_or(&self.wname)
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let (name, ext) = utils::file_stem_ext(&path)?;
        Ok(Media {
            wname: name.trim().to_lowercase(),
            new_name: String::new(),
            ext: utils::intern(ext),
            ts: fs::metadata(&path)?.created()?,
            smart_group: None,
            path,
        })
    }
}
