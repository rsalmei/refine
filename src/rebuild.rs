use crate::utils;
use anyhow::{anyhow, Result};
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
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
    /// Easily set filenames for new files. BEWARE: use only with new files on already organized folders.
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

pub fn rebuild(mut medias: Vec<Media>) -> Result<()> {
    println!("Rebuilding file names...");
    println!("  - strip before: {:?}", opt().strip_before);
    println!("  - strip after: {:?}", opt().strip_after);
    println!("  - strip exact: {:?}", opt().strip_exact);
    println!("  - smart detect: {}", !opt().no_smart_detect);
    println!("  - force: {:?}", opt().force);
    println!("  - interactive: {}", !opt().yes);
    println!();

    apply_strip(&mut medias, Pos::Before, &opt().strip_before)?;
    apply_strip(&mut medias, Pos::After, &opt().strip_after)?;
    apply_strip(&mut medias, Pos::Exact, &opt().strip_exact)?;
    if let Some(force) = &opt().force {
        medias
            .iter_mut()
            .filter(|m| m.wname.is_empty())
            .for_each(|m| {
                m.wname.clone_from(force);
            })
    }

    let total = medias.len();
    let (mut medias, mut empty) = medias
        .into_iter()
        .partition::<Vec<_>, _>(|m| !m.wname.is_empty());
    empty.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    empty.iter().for_each(|m| {
        eprintln!("warning: rules cleared name: {}", m.path.display());
    });

    if !opt().no_smart_detect {
        apply_smart_groups(&mut medias);
    }

    apply_new_names(&mut medias);
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

    let mut changes = medias
        .into_iter()
        .filter(|m| m.new_name != m.path.file_name().unwrap().to_str().unwrap()) // the list might have changed on force.
        .inspect(|m| {
            println!("{} --> {}", m.path.display(), m.new_name);
        })
        .collect::<Vec<_>>();

    if !changes.is_empty() || !empty.is_empty() {
        println!();
    }
    println!("total files: {total}");
    println!("  changes: {}", changes.len());

    if !changes.is_empty() && !opt().yes {
        utils::prompt_yes_no("apply changes?")?;
    }
    apply_renames(&mut changes);
    if changes.is_empty() {
        return Ok(());
    }

    println!("attempting to fix {} errors", changes.len());
    changes.iter_mut().for_each(|m| {
        let temp = format!("__refine+{}__", m.new_name);
        let dest = m.path.with_file_name(&temp);
        match fs::rename(&m.path, &dest) {
            Ok(()) => m.path = dest,
            Err(err) => eprintln!("error: {err:?}: {:?} --> {temp:?}", m.path),
        }
    });
    apply_renames(&mut changes);
    if !changes.is_empty() {
        println!("still {} errors, giving up", changes.len());
    }

    Ok(())
}

#[derive(Debug)]
enum Pos {
    Before,
    After,
    Exact,
}

fn apply_strip(medias: &mut [Media], pos: Pos, rules: &[String]) -> Result<()> {
    let (px, sx) = match pos {
        Pos::Before => (r"^.*", r"\s*"),
        Pos::After => (r"\s*", r".*$"),
        Pos::Exact => (r"\s*", r"\s*"),
    };
    for rule in rules {
        let re = Regex::new(&format!("(?i){px}{rule}{sx}"))?;
        medias.iter_mut().for_each(|m| {
            m.wname = re
                .split(&m.wname)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(""); // only actually used on Pos::Exact, the other two always return a single element.
        })
    }
    Ok(())
}

fn apply_smart_groups(medias: &mut [Media]) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[\s_]+").unwrap());

    medias.iter_mut().for_each(|m| {
        if let Cow::Owned(x) = re.replace_all(&m.wname, "") {
            m.smart_group = Some(x);
        }
    });
}

fn apply_new_names(medias: &mut [Media]) {
    medias.sort_unstable_by(|a, b| a.group().cmp(b.group()));
    medias
        .chunk_by_mut(|a, b| a.group() == b.group())
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
                false => base.to_owned(), // needed because g[m].name is borrowed, and I need to mutate it below.
            };
            g.iter_mut().enumerate().for_each(|(i, m)| {
                m.new_name.clear(); // because of the force option.
                write!(m.new_name, "{base}-{}.{}", i + 1, m.ext).unwrap();
            });
        });
}

fn apply_renames(changes: &mut Vec<Media>) {
    changes.retain(|m| {
        let dest = m.path.with_file_name(&m.new_name);
        if dest.exists() {
            eprintln!("error: path already exists: {dest:?}");
            return true;
        }
        match fs::rename(&m.path, &dest) {
            Ok(()) => false,
            Err(err) => {
                eprintln!("error: {err:?}: {:?} --> {:?}", m.path, m.new_name);
                true
            }
        }
    });
    if changes.is_empty() {
        println!("done");
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

impl Media {
    fn group(&self) -> &str {
        self.smart_group.as_deref().unwrap_or(&self.wname)
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let name = path
            .file_stem()
            .ok_or_else(|| anyhow!("no file name: {path:?}"))?
            .to_str()
            .ok_or_else(|| anyhow!("file name str: {path:?}"))?;
        let ext = path.extension().unwrap_or_default().to_str().unwrap_or("");

        let mut wname = name.trim().to_lowercase();
        let name = utils::strip_sequence(&wname);
        if name != wname {
            wname.truncate(name.len());
        }

        Ok(Media {
            wname,
            new_name: String::new(),
            ext: ext_cache(ext),
            ts: fs::metadata(&path)?.created()?,
            smart_group: None,
            path,
        })
    }
}

fn ext_cache(ext: &str) -> &'static str {
    static EXT: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let m = EXT.get_or_init(Default::default);

    let mut m = m.lock().unwrap();
    match m.get(ext) {
        Some(x) => x,
        None => {
            let ext = Box::leak(ext.to_owned().into_boxed_str());
            m.insert(ext);
            ext
        }
    }
}
