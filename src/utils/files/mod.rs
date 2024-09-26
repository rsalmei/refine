use anyhow::{anyhow, Context, Result};
pub use domain::*;
pub use ops::*;
use regex::Regex;
use std::borrow::Cow;
use std::path::Path;
use std::sync::LazyLock;

mod domain;
mod ops;

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

/// Strip parts of filenames, either before, after, or exactly a certain string.
pub fn strip_filenames(
    medias: &mut [impl NewNameMut],
    rules: [&[impl AsRef<str>]; 3],
) -> Result<()> {
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

    // apply all rules in order.
    medias.iter_mut().for_each(|m| {
        let mut name = std::mem::take(m.new_name_mut());
        regs.iter().for_each(|re| match re.replace_all(&name, "") {
            Cow::Borrowed(_) => {}
            Cow::Owned(x) => name = x,
        });
        *m.new_name_mut() = name;
    });
    Ok(())
}

/// Remove cleared filenames after applying some renaming rules.
pub fn remove_cleared(medias: &mut Vec<impl NewName + OriginalPath>) -> usize {
    medias.sort_unstable_by(|m, n| m.path().cmp(n.path()));
    let total = medias.len();
    medias.retain(|m| {
        if m.new_name().is_empty() {
            eprintln!("warning: rules cleared name: {}", m.path().display());
            false
        } else {
            true
        }
    });
    total - medias.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

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
        impl NewNameMut for Media {
            fn new_name_mut(&mut self) -> &mut String {
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

    #[test]
    fn cleared() {
        #[derive(Debug, PartialEq)]
        struct Media(String, PathBuf);
        impl NewName for Media {
            fn new_name(&self) -> &str {
                &self.0
            }
        }
        impl OriginalPath for Media {
            fn path(&self) -> &Path {
                &self.1
            }
        }

        let mut medias = vec![
            Media("".to_owned(), PathBuf::from("/2")),
            Media("bar".to_owned(), PathBuf::from("/2")),
            Media("".to_owned(), PathBuf::from("/3")),
            Media("foo".to_owned(), PathBuf::from("/1")),
            Media("".to_owned(), PathBuf::from("/1")),
        ];

        let cleared = remove_cleared(&mut medias);
        assert_eq!(cleared, 3);
        assert_eq!(
            medias,
            vec![
                Media("foo".to_owned(), PathBuf::from("/1")),
                Media("bar".to_owned(), PathBuf::from("/2"))
            ]
        );
    }
}
