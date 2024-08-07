use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{fs, io};

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
            StripPos::Exact => {
                &format!(r"(?i){BOUND}+{rule}$|^{rule}{BOUND}+|{BOUND}+{rule}|{rule}")
            }
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

/// Rename files and directories. Works only within the same file system.
/// Can also be used to move files and directories, when the target path is not the same.
pub fn rename_move_consuming(medias: &mut Vec<impl OriginalPath + NewPath>) {
    files_op(medias, silent, |p, q| fs::rename(p, q))
}

/// Copy files to a new location. Works between file systems.
pub fn copy_consuming(medias: &mut Vec<impl OriginalPath + NewPath>) {
    files_op(medias, verbose, |p, q| copy_path(p, q, false, 0))
}

/// Move files to a new location by copying and removing the original. Works between file systems.
pub fn cross_move_consuming(medias: &mut Vec<impl OriginalPath + NewPath>) {
    files_op(medias, verbose, |p, q| copy_path(p, q, true, 0))
}

fn copy_path(p: &Path, q: &Path, remove_dir: bool, n: usize) -> io::Result<()> {
    match p.is_dir() {
        true => fs::create_dir(q).and_then(|()| {
            verbose(b"d[");
            let files = fs::read_dir(p)?
                .flatten()
                .try_fold(Vec::new(), |mut acc, de| {
                    let is_dir = de.path().is_dir(); // need to cache is_dir because it goes to the fs again, and copy_path below may delete it.
                    copy_path(&de.path(), &q.join(de.file_name()), remove_dir, n + 1).map(|()| {
                        if !is_dir {
                            verbose(b".");
                            if remove_dir {
                                acc.push(de.path())
                            }
                        }
                        acc
                    })
                });
            verbose(b"]");
            match remove_dir {
                true => files
                    .and_then(|files| files.iter().try_for_each(fs::remove_file))
                    .and_then(|_| fs::remove_dir(p)),
                false => files.map(|_| ()),
            }
        }),
        false if n == 0 => fs::copy(p, q).and_then(|_| {
            verbose(b".");
            fs::remove_file(p)
        }),
        false => fs::copy(p, q).map(|_| ()), // this is called recursively by the is_dir case above.
    }
}

fn silent(_: &[u8]) {}
fn verbose(c: &[u8]) {
    io::stdout().write_all(c).unwrap();
    io::stdout().flush().unwrap();
}

type FileOp = fn(&Path, &Path) -> io::Result<()>;
fn files_op(paths: &mut Vec<impl OriginalPath + NewPath>, notify: fn(&[u8]), op: FileOp) {
    paths.retain(|m| {
        let target = m.new_path();
        if target.exists() {
            notify(b"-\n");
            eprintln!("file already exists: {:?} -> {target:?}", m.path());
            notify(b"\n");
            return true;
        }
        match op(m.path(), &target) {
            Ok(()) => false,
            Err(err) => {
                notify(b"x\n");
                eprintln!("error: {err}: {:?} -> {target:?}", m.path());
                notify(b"\n");
                true
            }
        }
    });
    notify(b"\n");
}
