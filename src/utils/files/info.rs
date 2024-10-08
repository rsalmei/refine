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
pub fn sequence(stem: &str) -> Option<Sequence> {
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

/// Determine the real length of a file stem without the sequence number.
pub fn real_length(stem: &str) -> usize {
    let len = stem.len();
    sequence(stem).map_or(len, |seq| len - seq.len)
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
        fn case(stem: &str, expected: impl Into<Option<Sequence>>, real: usize) {
            let seq = expected.into();
            assert_eq!(sequence(stem), seq.into());
            assert_eq!(real_length(stem), real);
        }

        case("foo", None, 3);
        case("foo123", None, 6);
        case("foo-bar", None, 7);
        case("foo-bar123", None, 10);
        case("foo-123 bar", None, 11);
        case("foo - bar", None, 9);
        case("foo(bar)", None, 8);
        case("foo (bar)", None, 9);

        case("foo-123", Sequence { len: 4, seq: 123 }, 3); // the sequence style used here.
        case("foo2 123", Sequence { len: 4, seq: 123 }, 4); // macOS "Keep both files" when copying.
        case("foo-bar copy", Sequence { len: 5, seq: 2 }, 7); // macOS first "Keep both files" when moving.
        case("foo copy 123", Sequence { len: 9, seq: 123 }, 3); // macOS from second onward when moving.
        case("foobar (123)", Sequence { len: 6, seq: 123 }, 6); // Windows.

        case("f-o-o 1", Sequence { len: 2, seq: 1 }, 5); // macOS won't generate "1", but we'll accept it.
        case("foo copy 1", Sequence { len: 7, seq: 1 }, 3); // macOS won't generate "1", but we'll accept it.
        case("foo (1)", Sequence { len: 4, seq: 1 }, 3); // Windows won't generate "1", but we'll accept it.
    }
}
