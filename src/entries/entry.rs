use anyhow::{Result, anyhow};
use regex::Regex;
use std::cmp::Ordering;
use std::convert::Into;
use std::env;
use std::fmt::{self, Display};
use std::fs::Metadata;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;
use yansi::{Paint, Style};

/// A file or directory entry that is guaranteed to have a valid UTF-8 representation.
#[derive(Debug, Clone, Eq)] // Hash, PartialEq, Ord, and PartialOrd are below.
pub struct Entry {
    path: PathBuf,
    is_dir: bool,
}

/// Create a new entry from a path, checking that it has a valid UTF-8 representation.
impl TryFrom<PathBuf> for Entry {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let is_dir = path.is_dir();
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
        // I could just check that the entire path is valid UTF-8, but I want to give better error messages.
        if let Some(pp) = path.parent() {
            // the root dir has no parent.
            pp.to_str()
                .ok_or_else(|| anyhow!("no UTF-8 parent: {pp:?}"))?;
        }
        Ok(Entry { path, is_dir })
    }
}

pub static ROOT: LazyLock<Entry> = LazyLock::new(|| Entry::try_new("/", true).unwrap());

impl Entry {
    /// Create a new entry that, in case the path does not exist, will assume the given directory flag.
    /// If it does exist, check that it has the correct directory flag or panic.
    pub fn try_new(path: impl Into<PathBuf>, is_dir: bool) -> Result<Self, anyhow::Error> {
        let path = path.into();
        if path.to_str().is_none() {
            return Err(anyhow!("invalid UTF-8 path: {path:?}"));
        }

        // panic if the entry exists and the directory flag doesn't match.
        // it should never happen in normal program logic, so if it does it's a bug.
        match path.try_exists() {
            Ok(true) => assert_eq!(path.is_dir(), is_dir, "is_dir error in {path:?}: {is_dir}"),
            Ok(false) => {} // the path was verified to not exist, cool.
            Err(err) => println!("warning: couldn't verify {path:?}: {err}"),
        }

        Ok(Entry { path, is_dir })
    }

    /// Create a new entry with the given name adjoined without checking UTF-8 again.
    pub fn join(&self, name: impl AsRef<str>) -> Entry {
        let path = self.path.join(name.as_ref());
        let is_dir = path.is_dir();
        Entry { path, is_dir }
    }

    /// Create a new entry with the given name without checking UTF-8 again.
    pub fn with_file_name(&self, name: impl AsRef<str>) -> Entry {
        let path = self.path.with_file_name(name.as_ref());
        let is_dir = path.is_dir();
        Entry { path, is_dir }
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

    /// Get the name, aliases, sequence, comment, and extension from collection media names.
    pub fn collection_parts(&self) -> (&str, Option<Vec<&str>>, Option<usize>, &str, &str) {
        // regex: name~24 or name+alias1,alias2~24 or just name.
        static RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^(\w+)(?:\+(\w+(?:,\w+)*))?~(\d+)(.+)?$").unwrap());

        let (stem, ext) = self.filename_parts();
        let Some(caps) = RE.captures(stem) else {
            return (stem, None, None, "", ext);
        };
        let name = caps.get(1).unwrap().as_str(); // regex guarantees name is present.
        let aliases = caps.get(2).map(|m| m.as_str().split(',').collect());
        let seq = caps.get(3).and_then(|m| m.as_str().parse().ok());
        let comment = caps.get(4).map_or("", |m| m.as_str());
        (name, aliases, seq, comment, ext)
    }

    /// Return a cached directory flag, which does not touch the filesystem again.
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

    pub fn metadata(&self) -> Result<Metadata> {
        self.path.metadata().map_err(Into::into)
    }

    pub fn starts_with(&self, prefix: impl AsRef<Path>) -> bool {
        self.path.starts_with(prefix)
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn display_path(&self) -> impl Display {
        DisplayPath(self)
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

/// A [Display] implementation for [Entry] that print its full path.
#[derive(Debug)]
pub struct DisplayPath<'a>(&'a Entry);

/// A [Display] implementation for [Entry] that print only its file name.
#[derive(Debug)]
pub struct DisplayFilename<'a>(&'a Entry);

const DIR_STYLE: (Style, Style) = {
    let parent_dir: Style = Style::new().yellow();
    (parent_dir, parent_dir.bold())
};
const FILE_STYLE: (Style, Style) = {
    let parent_file = Style::new().cyan();
    (parent_file, parent_file.bold())
};

impl Display for DisplayPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entry = self.0;
        let (parent, name, symbol) = display_parts(entry);
        let (p_style, n_style) = if entry.is_dir { DIR_STYLE } else { FILE_STYLE };
        write!(
            f,
            "{}{}{}",
            parent.paint(p_style),
            name.paint(n_style),
            symbol.paint(n_style)
        )
    }
}

impl Display for DisplayFilename<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entry = self.0;
        let (_, name, symbol) = display_parts(entry);
        let (_, style) = if entry.is_dir { DIR_STYLE } else { FILE_STYLE };
        write!(f, "{}{}", name.paint(style), symbol.paint(style))
    }
}

/// Get the parent directory, name, and directory symbol for an entry.
/// They are used by [DisplayPath] and [DisplayFilename] implementations, which style them.
fn display_parts(entry: &Entry) -> (&str, &str, &str) {
    let full = entry.to_str();
    let (parent, name) = match entry.path.file_name().map(|s| s.to_str().unwrap()) {
        Some(name) => {
            let pos = full.rfind(name).unwrap();
            (&full[..pos], name)
        }
        None => ("", full),
    };
    let dir_id = match entry.is_dir && !name.ends_with('/') {
        true => "/",
        false => "",
    };
    (parent, name, dir_id)
}

impl Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.display_path().fmt(f)
    }
}

impl AsRef<Path> for Entry {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl From<&Entry> for Entry {
    fn from(entry: &Entry) -> Self {
        entry.clone()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_creation() {
        #[track_caller]
        fn case(p: impl Into<PathBuf>) -> Result<Entry, anyhow::Error> {
            Entry::try_from(p.into())
        }

        case("foo").unwrap();
        case("foo.bar").unwrap();
        case("foo.bar.baz").unwrap();
        case("foo/").unwrap();
        case("😃").unwrap();
    }

    #[test]
    fn filename_parts() {
        #[track_caller]
        fn case(p: impl Into<PathBuf>, is_dir: bool, out: (&str, &str)) {
            let entry = Entry {
                path: p.into(),
                is_dir,
            };
            assert_eq!(out, entry.filename_parts())
        }

        case("foo", false, ("foo", ""));
        case("foo.bar", false, ("foo", "bar"));
        case("foo.bar.baz", false, ("foo.bar", "baz"));

        case(".foo", false, (".foo", ""));
        case(".foo.bar", false, (".foo", "bar"));
        case(".foo.bar.baz", false, (".foo.bar", "baz"));

        case("foo", true, ("foo", ""));
        case("foo.bar", true, ("foo.bar", ""));
        case("foo.bar.baz", true, ("foo.bar.baz", ""));

        case(".foo", true, (".foo", ""));
        case(".foo.bar", true, (".foo.bar", ""));
        case(".foo.bar.baz", true, (".foo.bar.baz", ""));
    }

    #[test]
    fn collection_parts() {
        #[track_caller]
        fn case(base: &str, out: (&str, Option<Vec<&str>>, Option<usize>, &str)) {
            let (name, aliases, seq, comment) = out;
            let entry = Entry::try_new(format!("{}.ext", base), false).unwrap();
            let out = (name, aliases, seq, comment, "ext");
            assert_eq!(out, entry.collection_parts());
        }

        // stem only.
        case("foo", ("foo", None, None, ""));
        case("foo bar", ("foo bar", None, None, ""));
        case("foo bar - baz", ("foo bar - baz", None, None, ""));
        case("foo - 2025 - 24", ("foo - 2025 - 24", None, None, ""));
        case("_foo_-24", ("_foo_-24", None, None, ""));
        case("foo ~ 24", ("foo ~ 24", None, None, ""));
        case("foo~ 24", ("foo~ 24", None, None, ""));
        case("foo+bar", ("foo+bar", None, None, ""));
        case("foo+bar,baz", ("foo+bar,baz", None, None, ""));
        case("foo+bar ~ 24", ("foo+bar ~ 24", None, None, ""));
        case("foo ~24", ("foo ~24", None, None, ""));
        case("foo bar~24", ("foo bar~24", None, None, ""));
        case("foo bar ~24", ("foo bar ~24", None, None, ""));
        case("_foo_ ~24", ("_foo_ ~24", None, None, ""));
        case("foo - 33~24", ("foo - 33~24", None, None, ""));
        case("foo+ ~24", ("foo+ ~24", None, None, ""));
        case("foo+ asd~24", ("foo+ asd~24", None, None, ""));
        case("foo+asd ~24", ("foo+asd ~24", None, None, ""));
        case("foo+~24", ("foo+~24", None, None, ""));
        case("foo+,~24", ("foo+,~24", None, None, ""));
        case("foo+bar,~24", ("foo+bar,~24", None, None, ""));
        case("foo+bar,~24 cool", ("foo+bar,~24 cool", None, None, ""));

        // name and seq.
        case("foo~24", ("foo", None, Some(24), ""));
        case("foo_~24", ("foo_", None, Some(24), ""));
        case("__foo~24", ("__foo", None, Some(24), ""));
        case("_foo__~24", ("_foo__", None, Some(24), ""));

        // name, aliases and seq.
        case("foo+bar~24", ("foo", Some(vec!["bar"]), Some(24), ""));
        case(
            "foo+bar,baz~24",
            ("foo", Some(vec!["bar", "baz"]), Some(24), ""),
        );

        // name, seq, and comment.
        case("foo~24cool", ("foo", None, Some(24), "cool"));
        case("foo~24 cool", ("foo", None, Some(24), " cool"));
        case("foo_~24-nice!", ("foo_", None, Some(24), "-nice!"));
        case("__foo~24 ?why?", ("__foo", None, Some(24), " ?why?"));
        case("_foo__~24 - cut", ("_foo__", None, Some(24), " - cut"));

        // name, aliases, seq, and comment.
        case(
            "foo+bar~24 seen 3 times",
            ("foo", Some(vec!["bar"]), Some(24), " seen 3 times"),
        );
        case(
            "foo+bar,baz~24 with comment!",
            ("foo", Some(vec!["bar", "baz"]), Some(24), " with comment!"),
        );
    }

    #[test]
    fn fn_display_parts() {
        #[track_caller]
        fn case(p: impl Into<PathBuf>, is_dir: bool, out: (&str, &str, &str)) {
            let entry = Entry {
                path: p.into(),
                is_dir,
            };
            assert_eq!(out, display_parts(&entry));
        }

        // Directory cases (fixed)
        case(".", true, ("", ".", "/"));
        case("..", true, ("", "..", "/"));
        case("/", true, ("", "/", ""));
        case("./", true, ("", "./", ""));
        case("../", true, ("", "../", ""));
        case("dir", true, ("", "dir", "/"));
        case("dir/", true, ("", "dir", "/"));
        case("dir/.", true, ("", "dir", "/"));
        case("./dir", true, ("./", "dir", "/"));
        case("./dir/", true, ("./", "dir", "/"));
        case("./dir/.", true, ("./", "dir", "/"));

        // File cases
        case("file.txt", false, ("", "file.txt", ""));
        case("./file.txt", false, ("./", "file.txt", ""));
        case("dir/file.txt", false, ("dir/", "file.txt", ""));
        case("./dir/file.txt", false, ("./dir/", "file.txt", ""));
        case(".hidden", false, ("", ".hidden", ""));
        case("./dir/.hidden", false, ("./dir/", ".hidden", ""));
    }
}
