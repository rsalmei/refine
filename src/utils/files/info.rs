use crate::utils::Sequence;
use anyhow::{anyhow, Result};
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

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

/// Extract the sequence number from a file stem.
pub fn sequence(stem: &str) -> Sequence {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?:[- ](\d+)| copy (\d+)| \((\d+)\))$").unwrap());

    let (len, num) = if let Some((full, [seq])) = RE.captures(stem).map(|caps| caps.extract()) {
        (full.len(), seq.parse().unwrap()) // regex checked.
    } else if stem.ends_with(" copy") {
        (5, 2) // macOS first "Keep both files" when moving has no sequence (see also the test).
    } else {
        (0, 1)
    };
    Sequence {
        num,
        real_len: stem.len() - len,
    }
}

/// Return the kind of path, handy for display purposes.
///
/// Beware this function touches the filesystem, checking it every time it is called.
pub fn kind(p: &Path) -> &str {
    match p.is_dir() {
        true => "/",
        false => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parts() {
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
    fn extract_sequence() {
        #[track_caller]
        fn case(stem: &str, num: usize, real_len: usize) {
            assert_eq!(sequence(stem), Sequence { num, real_len });
        }

        // no sequence is found.
        case("foo", 1, 3);
        case("foo123", 1, 6);
        case("foo-bar", 1, 7);
        case("foo-bar123", 1, 10);
        case("foo-123 bar", 1, 11);
        case("foo - bar", 1, 9);
        case("foo(bar)", 1, 8);
        case("foo (bar)", 1, 9);

        // sequence is found.
        case("foo-123", 123, 3); // the sequence style used here.
        case("foo2 123", 123, 4); // macOS "Keep both files" when copying.
        case("foo-bar copy", 2, 7); // macOS first "Keep both files" when moving.
        case("foo copy 123", 123, 3); // macOS from second onward when moving.
        case("foobar (123)", 123, 6); // Windows.

        // edge cases.
        case("f-o-o 1", 1, 5); // macOS won't generate "1", but we'll accept it.
        case("foo copy 1", 1, 3); // macOS won't generate "1", but we'll accept it.
        case("foo (1)", 1, 3); // Windows won't generate "1", but we'll accept it.
    }
}
