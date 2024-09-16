use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::borrow::Cow;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{fs, io};

/// Get the file stem and extension from files, or name from directories.
pub fn filename_parts(path: &Path) -> Result<(&str, &str)> {
    if path.is_dir() {
        let name = path
            .file_name()
            .ok_or_else(|| anyhow!("no file name: {path:?}"))?
            .to_str()
            .ok_or_else(|| anyhow!("no UTF-8 file name: {path:?}"))?;
        Ok((name, ""))
    } else {
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Sequence {
    pub len: usize,
    pub seq: u32,
}

/// Extract the sequence number from a file stem.
pub fn extract_sequence(stem: &str) -> Option<Sequence> {
    static RE_SEQ: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?:[- ](\d+)| copy (\d+)| \((\d+)\))$").unwrap());

    if stem.ends_with(" copy") {
        return Some(Sequence { len: 5, seq: 2 }); // macOS first "Keep both files" when moving has no sequence.
    }
    let (full, [seq]) = RE_SEQ.captures(stem).map(|caps| caps.extract())?;
    Some(Sequence {
        len: full.len(),
        seq: seq.parse().unwrap_or(1),
    })
}

#[derive(Debug)]
pub enum StripPos {
    Before,
    After,
    Exact,
}

pub trait NewName {
    fn new_name(&mut self) -> &mut String;
}

pub fn strip_filenames(medias: &mut [impl NewName], rules: [&[impl AsRef<str>]; 3]) -> Result<()> {
    const BOUND: &str = r"[-_\.\s]";
    let before = |rule| format!("(?i)^.*{rule}{BOUND}*");
    let after = |rule| format!("(?i){BOUND}*{rule}.*$");
    let exact = |rule| format!(r"(?i){BOUND}+{rule}$|^{rule}{BOUND}+|{BOUND}+{rule}|{rule}");

    // pre-compile all rules into regexes.
    let mut regs = Vec::with_capacity(rules.iter().map(|r| r.len()).sum());
    for (&group, regex) in rules.iter().zip([before, after, exact]) {
        for rule in group.iter().map(|x| x.as_ref()) {
            let regex = &regex(rule);
            let re = Regex::new(regex).with_context(|| format!("compiling regex: {rule:?}"))?;
            regs.push(re);
        }
    }
    let regs = regs;

    medias.iter_mut().for_each(|m| {
        let mut name = std::mem::take(m.new_name());
        regs.iter().for_each(|re| match re.replace_all(&name, "") {
            Cow::Borrowed(_) => {}
            Cow::Owned(x) => name = x,
        });
        *m.new_name() = name;
    });
    Ok(())
}

pub trait OriginalPath {
    /// The original path to the file.
    fn path(&self) -> &Path;
}

pub fn remove_cleared(medias: &mut Vec<impl NewName + OriginalPath>) -> usize {
    medias.sort_unstable_by(|m, n| m.path().cmp(n.path()));
    let mut warnings = 0;
    medias.retain_mut(|m| match m.new_name().is_empty() {
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn filename() {
        #[track_caller]
        fn case(p: impl AsRef<Path>, (s, e): (&str, &str)) {
            assert_eq!(filename_parts(p.as_ref()).unwrap(), (s, e));
        }

        case("foo", ("foo", ""));
        case("foo.bar", ("foo", "bar"));
        case("foo.bar.baz", ("foo.bar", "baz"));
        case("foo/", ("foo", ""));

        fs::create_dir("foo.bar").unwrap(); // not a great solution, but is_dir() actually tries the fs.
        case("foo.bar/", ("foo.bar", ""));
        fs::remove_dir("foo.bar").unwrap()
    }

    #[test]
    fn sequence() {
        #[track_caller]
        fn case(stem: &str, seq: impl Into<Option<Sequence>>) {
            assert_eq!(extract_sequence(stem), seq.into());
        }

        case("foo", None);
        case("foo123", None);
        case("foo-bar", None);
        case("foo-bar123", None);
        case("foo-123 bar", None);
        case("foo - bar", None);
        case("foo(bar)", None);
        case("foo (bar)", None);

        case("foo-123", Sequence { len: 4, seq: 123 }); // the sequence style used here.
        case("foo 123", Sequence { len: 4, seq: 123 }); // macOS "Keep both files" when copying.
        case("foo copy", Sequence { len: 5, seq: 2 }); // macOS first "Keep both files" when moving.
        case("foo copy 123", Sequence { len: 9, seq: 123 }); // macOS from second onward when moving.
        case("foo (123)", Sequence { len: 6, seq: 123 }); // Windows.

        case("foo 1", Sequence { len: 2, seq: 1 }); // macOS won't generate "1", but we'll accept it.
        case("foo copy 1", Sequence { len: 7, seq: 1 }); // macOS won't generate "1", but we'll accept it.
        case("foo (1)", Sequence { len: 4, seq: 1 }); // Windows won't generate "1", but we'll accept it.
    }

    #[test]
    fn strip() {
        struct Media(String);
        impl NewName for Media {
            fn new_name(&mut self) -> &mut String {
                &mut self.0
            }
        }

        #[track_caller]
        fn case(rules: [&[&str]; 3], stem: &str, new_name: &str) {
            let mut medias = vec![Media(stem.to_owned())];
            strip_filenames(&mut medias, rules).unwrap();
            assert_eq!(medias[0].0, new_name);
        }

        case([&["Before"], &[], &[]], "beforefoo", "foo");
        case([&["Before"], &[], &[]], "before foo", "foo");
        case([&["Before"], &[], &[]], "Before__foo", "foo");
        case([&["before"], &[], &[]], "Before - foo", "foo");
        case([&["before"], &[], &[]], "before.foo", "foo");
        case([&["before"], &[], &[]], "Before\t.  foo", "foo");

        case([&[], &["After"], &[]], "fooafter", "foo");
        case([&[], &["After"], &[]], "foo after", "foo");
        case([&[], &["After"], &[]], "foo__After", "foo");
        case([&[], &["after"], &[]], "foo - After", "foo");
        case([&[], &["after"], &[]], "foo.after", "foo");
        case([&[], &["after"], &[]], "foo\t. After", "foo");

        // exact: {BOUND}+{rule}$
        case([&[], &[], &["Exact"]], "foo exact", "foo");
        case([&[], &[], &["Exact"]], "foo__Exact", "foo");
        case([&[], &[], &["exact"]], "foo - Exact", "foo");
        case([&[], &[], &["exact"]], "foo.exact", "foo");
        case([&[], &[], &["exact"]], "foo\t. Exact", "foo");

        // exact: ^{rule}{BOUND}+
        case([&[], &[], &["Exact"]], "exact foo", "foo");
        case([&[], &[], &["Exact"]], "Exact__foo", "foo");
        case([&[], &[], &["exact"]], "Exact - foo", "foo");
        case([&[], &[], &["exact"]], "exact.foo", "foo");
        case([&[], &[], &["exact"]], "Exact\t.  foo", "foo");

        // exact: {BOUND}+{rule}
        case([&[], &[], &["Exact"]], "foo exact bar", "foo bar");
        case([&[], &[], &["Exact"]], "foo__Exact-bar", "foo-bar");
        case([&[], &[], &["exact"]], "foo - Exact_bar", "foo_bar");
        case([&[], &[], &["exact"]], "foo.exact.bar", "foo.bar");
        case([&[], &[], &["exact"]], "foo\t.  Exact - bar", "foo - bar");

        // exact: {rule}
        case([&[], &[], &["Exact"]], "fexactoo", "foo");
        case([&[], &[], &["Exact"]], "fexactoExacto", "foo");
        case([&[], &[], &["Exact"]], "fooExact bar", "foo bar");
        case([&[], &[], &["exact"]], "Exactfoo bar", "foo bar");

        // exact: unfortunate case, where I'd need lookahead to avoid it...
        // case([&[], &[], &["Exact"]], "foo Exactbar", "foo bar");
    }
}
