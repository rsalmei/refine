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

    apply_strip(&mut medias, Pos::Before, &opt().strip_before)?;
    apply_strip(&mut medias, Pos::After, &opt().strip_after)?;
    apply_strip(&mut medias, Pos::Exact, &opt().strip_exact)?;

    medias.iter_mut().for_each(|m| {
        let name = super::strip_sequence(&m.new_name);
        if name != m.new_name {
            m.new_name.truncate(name.len());
        }
    });

    if !opt().no_smart_detect {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r"[\s_]+").unwrap());

        medias.iter_mut().for_each(|m| {
            m.smart_group = match re.replace_all(&m.new_name, "") {
                Cow::Borrowed(_) => None,
                Cow::Owned(s) => Some(s),
            }
        });
    }

    medias.sort_unstable_by(|a, b| a.smart_group().cmp(b.smart_group()));
    medias
        .chunk_by_mut(|a, b| a.smart_group() == b.smart_group())
        .for_each(|g| {
            g.sort_by_key(|m| m.ts);
            let name = match opt().no_smart_detect {
                false => {
                    let vars = g.iter().map(|m| &m.new_name).collect::<HashSet<_>>();
                    vars.iter().map(|&x| (x.len(), x)).max().unwrap().1
                }
                true => &g[0].new_name,
            };
            let name = match name.contains(' ') {
                true => name.replace(' ', "_"),
                false => name.to_owned(), // needed because I'll clear new_name below.
            };
            g.iter_mut().enumerate().for_each(|(i, m)| {
                m.new_name.clear();
                write!(m.new_name, "{name}-{}.{}", i + 1, m.ext).unwrap();
            });
        });

    let changes = medias
        .iter()
        .filter(|m| m.new_name != m.path.file_name().unwrap().to_str().unwrap())
        .map(|m| {
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
        .count();

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
            m.new_name = re
                .split(&m.new_name)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "); // only actually used on Pos::Exact, the other two always return a single element.
        })
    }
    Ok(())
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
