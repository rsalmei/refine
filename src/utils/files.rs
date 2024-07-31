use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Get the file stem and extension from files, or name from directories.
pub fn filename_parts(path: &Path) -> Result<(&str, &str)> {
    match path.is_dir() {
        true => {
            let name = path
                .file_name()
                .ok_or_else(|| anyhow!("no file name: {path:?}"))?
                .to_str()
                .ok_or_else(|| anyhow!("no UTF-8 file name: {path:?}"))?;
            Ok((name, ""))
        }
        false => {
            let stem = path
                .file_stem()
                .ok_or_else(|| anyhow!("no file stem: {path:?}"))?
                .to_str()
                .ok_or_else(|| anyhow!("no UTF-8 file stem: {path:?}"))?;
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_str()
                .ok_or_else(|| anyhow!("no UTF-8 extension: {path:?}"))?;
            Ok((stem, ext))
        }
    }
}

/// Strip sequence numbers from a filename.
pub fn strip_sequence(name: &str) -> &str {
    static REM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" copy( \d+)?$").unwrap());
    static REL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-\d+$").unwrap());

    // replace_all() would allocate a new string, which would be a waste.
    let name = REM.split(name).next().unwrap(); // even if the name is " copy", this returns an empty str.
    REL.split(name).next().unwrap() // same as above, even if the name is "-1", this returns an empty str.
}

#[derive(Debug)]
pub enum StripPos {
    Before,
    After,
    Exact,
}

pub trait WorkingName {
    fn wname(&mut self) -> &mut String;
}

pub fn strip_names(medias: &mut [impl WorkingName], pos: StripPos, rules: &[String]) -> Result<()> {
    const BOUND: &str = r"[-_\.\s]";
    for rule in rules {
        let regex = match pos {
            StripPos::Before => &format!("(?i)^.*{rule}{BOUND}*"),
            StripPos::After => &format!("(?i){BOUND}*{rule}.*$"),
            StripPos::Exact => &format!(r"(?i){BOUND}+{rule}|^{rule}{BOUND}+|{rule}"),
        };
        let re = Regex::new(regex).with_context(|| format!("compiling regex: {rule:?}"))?;
        medias.iter_mut().for_each(|m| {
            *m.wname() = re
                .split(m.wname())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(""); // only actually used on Pos::Exact, the other two always return a single element.
        })
    }
    Ok(())
}

pub trait OriginalPath {
    /// The original path to the file.
    fn path(&self) -> &Path;
}

pub fn remove_cleared(medias: &mut Vec<impl WorkingName + OriginalPath>) -> usize {
    medias.sort_unstable_by(|m, n| m.path().cmp(n.path()));
    let mut warnings = 0;
    medias.retain_mut(|m| match m.wname().is_empty() {
        true => {
            warnings += 1;
            eprintln!("warning: rules cleared name: {}", m.path().display());
            false
        }
        false => true,
    });
    warnings
}

pub trait NewPath {
    /// The original path to the file.
    fn new_path(&self) -> PathBuf;
}

pub fn rename_consuming(files: &mut Vec<impl OriginalPath + NewPath>) {
}

    files.retain(|m| {
        let dest = m.path().with_file_name(m.new_name());
        if dest.exists() {
            eprintln!("error: file already exists: {dest:?}");
            return true;
        }
        match fs::rename(m.path(), &dest) {
            Ok(()) => false,
            Err(err) => {
                eprintln!("error: {err:?}: {:?} --> {:?}", m.path(), m.new_name());
                true
            }
        }
    });
}
