use crate::utils;
use crate::utils::{NewNameMut, OriginalPath};
use anyhow::{Context, Result};
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use regex::Regex;
use std::borrow::Cow;
use std::iter;

/// A set of rules that allows the user to customize filenames.
#[derive(Debug, Args)]
pub struct NamingRules {
    /// Strip from the start of the filename; blanks nearby are automatically removed.
    #[arg(short = 'b', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_before: Vec<String>,
    /// Strip to the end of the filename; blanks nearby are automatically removed.
    #[arg(short = 'a', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_after: Vec<String>,
    /// Strip all occurrences in the filename; blanks nearby are automatically removed.
    #[arg(short = 'e', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_exact: Vec<String>,
    /// Replace all occurrences in the filename with another; blanks are not touched.
    #[arg(short = 'r', long, value_name = "STR|REGEX=STR|$N", allow_hyphen_values = true, value_parser = utils::parse_key_value::<String, String>)]
    replace: Vec<(String, String)>,
}

impl NamingRules {
    /// Strip and replace parts of filenames based on the given rules.
    ///
    /// Return the number of warnings generated.
    pub fn apply<M: NewNameMut + OriginalPath>(
        &self,
        medias: &mut Vec<M>,
        mark_changed: impl Fn(&mut M, bool),
    ) -> Result<usize> {
        apply_rules(
            [&self.strip_before, &self.strip_after, &self.strip_exact],
            &self.replace,
            medias,
            mark_changed,
        )
    }
}

fn apply_rules<M: NewNameMut + OriginalPath>(
    strip_rules: [&[impl AsRef<str>]; 3],
    replace_rules: &[(impl AsRef<str>, impl AsRef<str>)],
    medias: &mut Vec<M>,
    mark_changed: impl Fn(&mut M, bool),
) -> Result<usize> {
    const BOUND: &str = r"[-_\.\s]";
    let before = |rule| format!("(?i)^.*{rule}{BOUND}*");
    let after = |rule| format!("(?i){BOUND}*{rule}.*$");
    let exactly = |rule| format!(r"(?i){BOUND}+{rule}$|^{rule}{BOUND}+|{BOUND}+{rule}|{rule}");
    let replace = |rule| format!(r"(?i){rule}");

    // pre-compile all rules into regexes.
    let regs = {
        let num = strip_rules.iter().map(|g| g.len()).sum::<usize>() + replace_rules.len();
        let mut regs = Vec::with_capacity(num);
        let rules = strip_rules
            .into_iter()
            .map(|g| g.iter().map(|r| (r.as_ref(), "")).collect::<Vec<_>>())
            .chain(iter::once(
                replace_rules
                    .iter()
                    .map(|(k, v)| (k.as_ref(), v.as_ref()))
                    .collect(),
            ))
            .zip([before, after, exactly, replace])
            .flat_map(|(g, f)| g.into_iter().map(move |(k, v)| (k, v, f)));
        for (rule, to, f) in rules {
            let re = Regex::new(&f(rule)).with_context(|| format!("compiling regex: {rule:?}"))?;
            regs.push((re, to));
        }
        regs
    };

    // this is just so that warnings are printed in a consistent order.
    medias.sort_unstable_by(|m, n| m.path().cmp(n.path()));

    // apply all rules in order.
    let total = medias.len();
    medias.retain_mut(|m| {
        let mut changed = false;
        let mut name = std::mem::take(m.new_name_mut());
        regs.iter().for_each(|(re, to)| {
            if let Cow::Owned(x) = re.replace_all(&name, *to) {
                changed = true;
                name = x
            }
        });

        if name.is_empty() {
            eprintln!("warning: rules cleared name: {}", m.path().display());
            false
        } else {
            *m.new_name_mut() = name;
            mark_changed(m, changed);
            true
        }
    });
    Ok(total - medias.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const NO_STRIP: [&[&str]; 3] = [&[], &[], &[]];
    const NO_REPLACE: &[(&str, &str)] = &[];

    /// A dummy type that expects it is always changed.
    #[derive(Debug, PartialEq)]
    struct Media(String);
    impl NewNameMut for Media {
        fn new_name_mut(&mut self) -> &mut String {
            &mut self.0
        }
    }
    impl OriginalPath for Media {
        fn path(&self) -> &Path {
            "".as_ref()
        }
    }

    #[test]
    fn strip_rules() {
        #[track_caller]
        fn case(rules: [&[&str]; 3], stem: &str, new_name: &str) {
            let mut medias = vec![Media(stem.to_owned())];
            let res = apply_rules(rules, NO_REPLACE, &mut medias, |_, changed| {
                assert!(changed)
            });
            assert_eq!(res.unwrap(), 0);
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
    fn replace_rules() {
        #[track_caller]
        fn case(rules: &[(&str, &str)], stem: &str, new_name: &str) {
            let mut medias = vec![Media(stem.to_owned())];
            let res = apply_rules(NO_STRIP, rules, &mut medias, |_, changed| assert!(changed));
            assert_eq!(res.unwrap(), 0);
            assert_eq!(medias[0].0, new_name);
        }

        case(&[("-+", "-")], "foo---bar", "foo-bar");
        case(&[(r"(\w+) +(\w+)", "$2 $1")], "foo  bar", "bar foo");
        case(&[(r"(.+)(S0\dE0\d)", "$2.$1")], "fooS03E05", "S03E05.foo");
    }

    #[test]
    fn mark() {
        let stems = &["bfoo1", "foo2a", "fioo3", "fuu4", "nope"];
        let expected = &["foo1", "foo2", "foo3", "foo4", "nope"];
        let mut medias = stems
            .iter()
            .map(|&s| Media(s.to_owned()))
            .collect::<Vec<_>>();
        let res = apply_rules(
            [&["b"], &["a"], &["i"]],
            &[("u", "o")],
            &mut medias,
            |m, changed| assert_eq!(changed, m.0 != "nope"),
        );
        assert_eq!(res.unwrap(), 0);
        assert_eq!(
            medias.iter().map(|m| &m.0).collect::<Vec<_>>(),
            expected.to_vec()
        );
    }

    #[test]
    fn cleared() {
        let mut medias = vec![
            Media("file".to_owned()),
            Media("batch".to_owned()),
            Media("collection".to_owned()),
            Media("refine".to_owned()),
            Media("foobar".to_owned()),
        ];

        let res = apply_rules(
            [&["e"], &["b"], &["c.*i"]],
            &[("on", "")],
            &mut medias,
            |_, changed| assert!(changed),
        );
        assert_eq!(res.unwrap(), 4);
        assert_eq!(medias, vec![Media("foo".to_owned())]);
    }
}