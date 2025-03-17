use super::sequence::Sequence;
use anyhow::{Result, anyhow};
use std::cmp::Ordering;
use std::fmt::{self, Display};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use yansi::{Paint, Style};

/// A file or directory entry that is guaranteed to have a valid UTF-8 representation.
#[derive(Debug, Clone)]
pub struct Entry {
    path: PathBuf,
    is_dir: bool,
}

/// Create a new entry from a path, checking that it has a valid UTF-8 representation.
impl TryFrom<PathBuf> for Entry {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let is_dir = path.is_dir();
        if is_dir {
            path.file_name()
                .ok_or_else(|| anyhow!("no dir name: {path:?}"))?
                .to_str()
                .ok_or_else(|| anyhow!("no UTF-8 dir name: {path:?}"))?;
        } else {
            path.file_stem()
                .ok_or_else(|| anyhow!("no file stem: {path:?}"))?
                .to_str()
                .ok_or_else(|| anyhow!("no UTF-8 file stem: {path:?}"))?;
            path.extension()
                .unwrap_or_default()
                .to_str()
                .ok_or_else(|| anyhow!("no UTF-8 file extension: {path:?}"))?;
        }
        // yes, I could just check that the entire path is valid UTF-8, but I want to give better error messages.
        path.parent()
            .ok_or_else(|| anyhow!("no parent: {path:?}"))?
            .to_str()
            .ok_or_else(|| anyhow!("no UTF-8 parent: {path:?}"))?;
        Ok(Entry { path, is_dir })
    }
}

impl Entry {
    /// Create a new entry that, in case the path does not exist, will assume the given directory flag.
    /// If it does exist, check that it has the correct directory flag or panic.
    pub fn new(path: PathBuf, is_dir: bool) -> Result<Self> {
        let mut entry = Self::try_from(path)?;
        match entry.path.exists() {
            true => assert_eq!(entry.is_dir, is_dir),
            false => entry.is_dir = is_dir,
        }
        Ok(entry)
    }

    /// Get the stem and extension from files, or name from directories.
    pub fn filename_parts(&self) -> (&str, &str) {
        match self.is_dir {
            true => (self.file_name(), ""),
            false => (
                self.path.file_stem().unwrap().to_str().unwrap(),
                self.path.extension().unwrap_or_default().to_str().unwrap(),
            ),
        }
    }

    /// Get the name, sequence, and extension from collection medias.
    pub fn collection_parts(&self) -> (&str, Option<usize>, &str) {
        // static RE: LazyLock<Regex> =
        //     LazyLock::new(|| Regex::new(r"^(?<n>[^ ]*) \((?<a>.*)\)$").unwrap());

        assert!(!self.is_dir, "not a file: {self}");
        let (stem, ext) = self.filename_parts();
        let seq = Sequence::from(stem);
        let name = &stem[..seq.true_len];
        // let (name, alias) = match RE.captures(name).map(|caps| caps.extract()) {
        //     Some((name, [alias])) => (name, alias),
        //     _ => (name, ""),
        // };
        (name, seq.num, ext)
    }

    /// Return a cached directory flag, without touching the filesystem again.
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    /// Get the filename from entries directly as a &str.
    pub fn file_name(&self) -> &str {
        self.path
            .file_name()
            .map(|n| n.to_str().unwrap())
            .unwrap_or_default()
    }

    pub fn to_str(&self) -> &str {
        self.path.to_str().unwrap()
    }

    /// Get the parent directory as an entry, without checking UTF-8 again.
    pub fn parent(&self) -> Option<Entry> {
        self.path.parent().map(|p| Entry {
            path: p.to_owned(),
            is_dir: true,
        })
    }

    /// Get a new entry with the given file name, without checking UTF-8 again.
    pub fn with_file_name(&self, name: impl AsRef<str>) -> Entry {
        Entry {
            path: self.path.with_file_name(name.as_ref()),
            is_dir: self.is_dir,
        }
    }

    /// Get a new entry with the given name adjoined, without checking UTF-8 again.
    pub fn join(&self, name: impl AsRef<str>) -> Entry {
        Entry {
            path: self.path.join(name.as_ref()),
            is_dir: self.is_dir,
        }
    }

    pub fn display_path(&self) -> DisplayPath {
        DisplayPath(self)
    }

    pub fn display_filename(&self) -> DisplayFilename {
        DisplayFilename(self)
    }
}

/// A [Display] implementation for [Entry] that prints its full path.
#[derive(Debug)]
pub struct DisplayPath<'a>(&'a Entry);

/// A [Display] implementation for [Entry] that prints only its file name.
#[derive(Debug)]
pub struct DisplayFilename<'a>(&'a Entry);

const PAR_FILE: Style = Style::new().cyan();
const PAR_DIR: Style = Style::new().yellow();
const DIR: Style = PAR_DIR.bold();
const FILE: Style = PAR_FILE.bold();

impl Display for DisplayPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entry = self.0;
        let (par, sep) = entry
            .path
            .parent()
            .map(|p| p.to_str().unwrap())
            .map(|s| (s, if s == "/" { "" } else { "/" }))
            .unwrap_or_default();
        let name = entry.file_name();
        let (dir, file, sep2, style) = match entry.is_dir {
            true => (name, "", "/", PAR_DIR),
            false => ("", name, "", PAR_FILE),
        };
        write!(
            f,
            "{}{}{}{}{}",
            par.paint(style),
            sep.paint(style),
            dir.paint(DIR),
            sep2.paint(DIR),
            file.paint(FILE)
        )
    }
}

impl Display for DisplayFilename<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entry = self.0;
        let name = entry.file_name();
        let (style, kind) = match entry.is_dir {
            true => (DIR, "/"),
            false => (FILE, ""),
        };
        write!(f, "{}{}", name.paint(style), kind.paint(style))
    }
}

impl Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.display_path().fmt(f)
    }
}

impl Deref for Entry {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.path
    }
}

impl DerefMut for Entry {
    fn deref_mut(&mut self) -> &mut PathBuf {
        &mut self.path
    }
}

impl AsRef<Path> for Entry {
    fn as_ref(&self) -> &Path {
        self.deref()
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path.cmp(&other.path)
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for Entry {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_creation() {
        #[track_caller]
        fn case(p: impl AsRef<Path>) -> Result<Entry> {
            Entry::try_from(p.as_ref().to_owned())
        }

        case("foo").unwrap();
        case("foo.bar").unwrap();
        case("foo.bar.baz").unwrap();
        case("foo/").unwrap();
        case("😃").unwrap();

        case("a\0\0").unwrap_err();
    }

    #[test]
    fn filename_parts() {
        #[track_caller]
        fn case(p: impl AsRef<Path>, is_dir: bool, out: (&str, &str)) {
            let entry = Entry {
                path: p.as_ref().to_owned(),
                is_dir,
            };
            assert_eq!(out, entry.filename_parts())
        }

        case("foo", false, ("foo", ""));
        case("foo.bar", false, ("foo", "bar"));
        case("foo.bar.baz", false, ("foo.bar", "baz"));

        case("foo", true, ("foo", ""));
        case("foo.bar", true, ("foo.bar", ""));
        case("foo.bar.baz", true, ("foo.bar.baz", ""));
    }

    #[test]
    fn collection_parts() {
        #[track_caller]
        fn case(p: impl AsRef<Path>, out: (&str, Option<usize>, &str)) {
            let entry = Entry {
                path: p.as_ref().to_owned(),
                is_dir: false,
            };
            assert_eq!(out, entry.collection_parts())
        }

        case("foo", ("foo", None, ""));
        case("foo.bar", ("foo", None, "bar"));
        case("foo.bar.baz", ("foo.bar", None, "baz"));
        case("foo-1.bar.baz", ("foo-1.bar", None, "baz"));

        case("foo-1", ("foo", Some(1), ""));
        case("foo-1.bar", ("foo", Some(1), "bar"));
        case("foo.bar-1.baz", ("foo.bar", Some(1), "baz"));
        case("foo-1.bar-2.baz", ("foo-1.bar", Some(2), "baz"));
    }
}
