use anyhow::{anyhow, Result};
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::SystemTime;

#[derive(Debug, Args)]
pub struct Rebuild {
    /// Remove from the start of the filename to this str; blanks are automatically removed.
    #[arg(short = 'b', long)]
    pub strip_before: Vec<String>,
    /// Remove from this str to the end of the filename; blanks are automatically removed.
    #[arg(short = 'a', long)]
    pub strip_after: Vec<String>,
    /// Remove all occurrences of this str in the filename; blanks are automatically removed.
    #[arg(short = 'e', long)]
    pub strip_exact: Vec<String>,
    /// Detects and fixes similar filenames (e.g. "foo bar.mp4" and "foo__bar.mp4").
    #[arg(short = 's', long)]
    pub no_smart_detect: bool,
    /// Do not touch the filesystem, just print what would be done.
    #[arg(long)]
    pub dry_run: bool,
    /// Easily set filenames for new files. BEWARE: use only with new files on already organized folders.
    #[arg(short, long, value_name = "STR", value_parser = NonEmptyStringValueParser::new())]
    pub force: Option<String>,
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
    println!("  - dry run: {}", opt().dry_run);
    println!("  - force: {:?}", opt().force);

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
            if !opt().dry_run {
                let dest = m.path.with_file_name(&m.new_name);
                if dest.exists() {
                    println!("  EXISTS {:?}", m.path);
                } else if let Err(e) = fs::rename(&m.path, &dest) {
                    println!("  FAILED: {e:?}");
                }
            }
        })
        .collect::<Vec<_>>();

    println!("\ntotal files: {}", medias.len());
    println!("  changes: {changes}");
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
#[derive(Debug)]
pub struct Media {
    path: PathBuf,
    new_name: String,
    ext: String,
    smart_group: Option<String>,
    ts: SystemTime,
}

impl Media {
    fn smart_group(&self) -> &str {
        self.smart_group.as_deref().unwrap_or(&self.new_name)
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let name = path
            .file_name()
            .ok_or_else(|| anyhow!("no file name: {path:?}"))?
            .to_str()
            .ok_or_else(|| anyhow!("file name str: {path:?}"))?;
        let (name, ext) = name.split_once('.').unwrap_or((name, ""));
        let (_, ext) = ext.rsplit_once('.').unwrap_or(("", ext));
        Ok(Media {
            ts: fs::metadata(&path)?.created()?,
            new_name: name.trim().to_lowercase(),
            ext: ext.to_lowercase(),
            smart_group: None,
            path,
        })
    }
}
