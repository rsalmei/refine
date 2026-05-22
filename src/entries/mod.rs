mod display;
mod parts;

use crate::warning;
use anyhow::{Result, anyhow};
use display::{DisplayFilename, DisplayInvertedPath};
use std::cmp::Ordering;
use std::convert::Into;
use std::env;
use std::fmt::Display;
use std::fs::Metadata;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, OnceLock};

pub use parts::collection_parts;

/// The length of the common prefix of all input paths, used to display shorter paths in the output.
static PREFIX_LEN: OnceLock<usize> = OnceLock::new();

/// A file or directory entry that is guaranteed to have a valid UTF-8 representation.
#[derive(Debug, Clone, Eq)] // Hash, PartialEq, Ord, and PartialOrd are below.
pub struct Entry {
    path: PathBuf,
    is_dir: bool,
}

/// Create a new entry from a path, checking that it has a valid UTF-8 representation.
///
/// The path must exist.
impl TryFrom<PathBuf> for Entry {
    type Error = (PathBuf, anyhow::Error);

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        fn check_utf8(path: &Path) -> Result<bool> {
            let is_dir = path.metadata()?.is_dir(); // verify that the path exists and is a directory.
            // I could just check that the entire path is valid UTF-8, but I want to give more specific error messages, so I check the parts separately.
            if is_dir {
                path.file_name()
                    .unwrap_or_default() // the root dir has no name.
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
            // the root dir has no parent.
            if let Some(pp) = path.parent() {
                pp.to_str()
                    .ok_or_else(|| anyhow!("no UTF-8 parent: {pp:?}"))?;
            }
            Ok(is_dir)
        }
        match check_utf8(&path) {
            Ok(is_dir) => Ok(Entry { path, is_dir }),
            Err(err) => Err((path, err)),
        }
    }
}

pub static ROOT: LazyLock<Entry> = LazyLock::new(|| Entry::try_new("/".into(), true).unwrap());

impl Entry {
    /// Create a new entry that, in case the path does not exist, will assume the given directory flag.
    /// If it does exist, check that it has the correct directory flag or panic.
    pub fn try_new(path: PathBuf, is_dir: bool) -> Result<Self> {
        if path.to_str().is_none() {
            return Err(anyhow!("invalid UTF-8 path: {path:?}"));
        }

        // panic if the entry exists and the directory flag doesn't match.
        // it should never happen in normal program logic, so if it does it's a bug.
        match path.try_exists() {
            Ok(true) => assert_eq!(path.is_dir(), is_dir, "is_dir error in {path:?}: {is_dir}"),
            Ok(false) => {} // the path was verified to not exist, cool.
            Err(err) => warning!("couldn't verify {path:?}: {err}"),
        }

        Ok(Entry { path, is_dir })
    }

    /// Create a new entry with the given name adjoined without checking UTF-8 again.
    pub fn join(&self, name: impl AsRef<str>) -> Entry {
        let path = self.path.join(name.as_ref());
        let is_dir = path.is_dir(); // this does not guarantee that the path exists, but it does check that if it does exist, it has the correct directory flag.
        Entry { path, is_dir }
    }

    /// Get the stem and extension from files, or name from directories.
    pub fn filename_parts(&self) -> (&str, &'static str) {
        parts::filename_parts(self)
    }

    pub fn collection_parts(&self) -> (&str, Option<&str>, Option<usize>, &str) {
        let (stem, _) = self.filename_parts();
        collection_parts(stem)
    }

    /// Create a new file entry with the given name without checking UTF-8 again.
    pub fn with_file_name(&self, name: impl AsRef<str>) -> Entry {
        assert!(!self.is_dir, "can't join file name to a directory entry");
        Entry {
            path: self.path.with_file_name(name.as_ref()),
            is_dir: false,
        }
    }

    /// Return a cached directory flag, which does not touch the filesystem again.
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    /// Get the filename from entries directly as a &str.
    pub fn file_name(&self) -> &str {
        self.path
            .file_name()
            .map(|s| s.to_str().unwrap())
            .unwrap_or_default()
    }

    /// The length of the common prefix of all input paths, set once at the beginning of the
    /// program, with the common prefix length of all input paths.
    ///
    /// This is used to display shorter paths in the output, by slicing off the common prefix.
    pub fn prefix_len(len: usize) {
        PREFIX_LEN.set(len).unwrap();
    }

    /// Get the path as a &str, slicing off the common prefix if it was set, and trimming trailing
    /// slashes for consistent display.
    pub fn to_str(&self) -> &str {
        let prefix_len = *PREFIX_LEN.get().unwrap_or(&0);
        let full_str = self.path.to_str().unwrap().trim_end_matches('/');
        &full_str[prefix_len..]
    }

    /// Get the parent directory as an entry, without checking UTF-8 again.
    pub fn parent(&self) -> Option<Entry> {
        self.path.parent().map(|p| Entry {
            path: p.to_owned(),
            is_dir: true,
        })
    }

    pub fn metadata(&self) -> Result<Metadata> {
        self.path.metadata().map_err(Into::into)
    }

    pub fn display_inverted_path(&self) -> impl Display {
        DisplayInvertedPath(self)
    }

    pub fn display_filename(&self) -> impl Display {
        DisplayFilename(self)
    }

    pub fn resolve(&self) -> Result<Entry> {
        let mut it = self.path.components();
        let mut res = match it.next().unwrap() {
            Component::Normal(x) if x == "~" => {
                dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?
            }
            Component::Normal(x) => {
                let mut dir = env::current_dir()?;
                dir.push(x);
                dir
            }
            Component::CurDir => env::current_dir()?,
            Component::ParentDir => {
                let mut dir = env::current_dir()?;
                dir.pop();
                dir
            }
            x => PathBuf::from(x.as_os_str()),
        };
        for comp in it {
            match comp {
                Component::RootDir => res.push(comp), // windows might have returned Prefix above, so RootDir comes here.
                Component::Normal(_) => res.push(comp),
                Component::ParentDir => {
                    if !res.pop() {
                        return Err(anyhow!("invalid path: {self}"));
                    }
                }
                _ => unreachable!(),
            }
        }
        Entry::try_new(res, self.is_dir) // the paths prepended above are NOT guaranteed to be valid UTF-8.
    }
}

impl Deref for Entry {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl AsRef<Path> for Entry {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Hash for Entry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state)
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
