use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn file_stem_ext(path: &PathBuf) -> Result<(&str, &str)> {
    let stem = path
        .file_stem()
        .ok_or_else(|| anyhow!("no file name: {path:?}"))?
        .to_str()
        .ok_or_else(|| anyhow!("file name str: {path:?}"))?;
    let ext = path.extension().unwrap_or_default().to_str().unwrap_or("");
    Ok((stem, ext))
}

/// Strip sequence numbers from a filename.
pub fn strip_sequence(name: &str) -> &str {
    static RE_MULTI_MACOS: OnceLock<Regex> = OnceLock::new();
    static RE_MULTI_LOCAL: OnceLock<Regex> = OnceLock::new();
    let rem = RE_MULTI_MACOS.get_or_init(|| Regex::new(r" copy( \d+)?$").unwrap());
    let rel = RE_MULTI_LOCAL.get_or_init(|| Regex::new(r"-\d+$").unwrap());

    // replace_all() would allocate a new string, which would be a waste.
    let name = rem.split(name).next().unwrap(); // even if the name is " copy", this returns an empty str.
    rel.split(name).next().unwrap() // same as above, even if the name is "-1", this returns an empty str.
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
    const BOUND: &str = r"[-_\.\s()]";
    for rule in rules {
        let regex = match pos {
            StripPos::Before => &format!("(?i)^.*{rule}{BOUND}*"),
            StripPos::After => &format!("(?i){BOUND}*{rule}.*$"),
            StripPos::Exact => &format!(r"(?i){BOUND}+{rule}\b|\b{rule}{BOUND}+|{rule}"),
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

pub trait PathWorkingName: WorkingName {
    /// The original path to the file.
    fn path(&self) -> &Path;
}

pub fn remove_cleared(medias: &mut Vec<impl PathWorkingName>) -> bool {
    medias.sort_unstable_by(|m, n| m.path().cmp(n.path()));
    let mut warnings = false;
    medias.retain_mut(|m| match m.wname().is_empty() {
        true => {
            warnings = true;
            eprintln!("warning: rules cleared name: {}", m.path().display());
            false
        }
        false => true,
    });
    warnings
}

pub trait NewNamePathWorkingName: PathWorkingName {
    /// The new name of the file, including the extension.
    fn new_name(&self) -> &str;
}

pub fn rename_consuming(files: &mut Vec<impl NewNamePathWorkingName>) {
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
