use super::{NewNameMut, SourceEntry};
use crate::utils;
use anyhow::{Context, Result};
use clap::Args;
use clap::builder::NonEmptyStringValueParser;
use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

/// A set of rules that allows the user to customize filenames.
#[derive(Debug, Args)]
pub struct NamingSpec {
    /// Strip from the start till occurrence; includes separators nearby, use {S} if needed.
    #[arg(short = 'b', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_before: Vec<String>,
    /// Strip from occurrence till the end; includes separators nearby, use {S} if needed.
    #[arg(short = 'a', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_after: Vec<String>,
    /// Strip exact occurrences; includes separators nearby, use {S} if needed.
    #[arg(short = 'e', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    strip_exact: Vec<String>,
    /// Replace occurrences in the filename; separators are not touched, use {S} if needed.
    #[arg(short = 'r', long, value_name = "STR|REGEX=STR|$N", allow_hyphen_values = true, value_parser = utils::parse_key_value::<String, String>)]
    replace: Vec<(String, String)>,
    /// recipe: Downgrade some prefix to a suffix; use {S} if needed.
    #[arg(short = 'w', long, value_name = "STR|REGEX=STR", allow_hyphen_values = true, value_parser = utils::parse_key_value::<String, String>)]
    downgrade: Vec<(String, String)>,
}

impl NamingSpec {
    /// Compile this set of rules.
    pub fn compile(&self) -> Result<NamingRules> {
        NamingRules::compile(
            [&self.strip_before, &self.strip_after, &self.strip_exact],
            &self.replace,
            &self.downgrade,
        )
    }
}

#[derive(Debug)]
pub struct NamingRules(Vec<(Regex, String)>);

impl NamingRules {
    fn compile(
        strip_rules: [&[impl AsRef<str>]; 3],
        replace_rules: &[(impl AsRef<str>, impl AsRef<str>)],
        downgrade_rules: &[(impl AsRef<str>, impl AsRef<str>)],
    ) -> Result<NamingRules> {
        const O: &str = r"[(\[{]"; // enclosing opening.
        const C: &str = r"[)\]}]"; // enclosing closing.
        const SEP: &str = r"[-\s.,@]";
        let before = |rule| format!("^.*{rule}{C}*{SEP}*");
        let after = |rule| format!("{SEP}*{O}*{rule}.*$");
        let exact = |rule| {
            static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w$").unwrap());
            let b = if RE.is_match(rule) { r"\b" } else { r"\B" };
            format!(
                r"^{O}*{rule}{C}*{SEP}+|{SEP}+{O}*{rule}{C}*$|{SEP}+{O}*{rule}{C}*{b}|{O}*{rule}{C}*"
            )
        };
        let replace_key = |rule: &str| rule.to_owned();
        let downgrade_key = |rule| format!(r"^{rule}{SEP}+(.+)$");
        let downgrade_value = |val| format!(r"$1 - {val}");

        let rules = strip_rules
            .into_iter()
            .map(|g| {
                g.iter()
                    .map(|r| (r.as_ref(), String::new()))
                    .collect::<Vec<_>>()
            })
            .chain([replace_rules
                .iter()
                .map(|(k, v)| (k.as_ref(), v.as_ref().to_owned()))
                .collect()])
            .chain([downgrade_rules
                .iter()
                .map(|(k, v)| (k.as_ref(), downgrade_value(v.as_ref())))
                .collect()])
            .zip([before, after, exact, replace_key, downgrade_key])
            .flat_map(|(g, f)| g.into_iter().map(move |(k, v)| (k, v, f)))
            .map(|(rule, to, f)| {
                Regex::new(&format!(
                    "(?i){}",
                    f(rule).replace("{S}", "{S}*").replace("{S}", SEP)
                ))
                .with_context(|| format!("compiling regex: {rule:?}"))
                .map(|re| (re, to))
            })
            .collect::<Result<_>>()?;
        Ok(NamingRules(rules))
    }

    /// Apply these rules to a list of media, consuming the entries that got their names cleared.
    ///
    /// The [NewNameMut] is used as the starting point, and is mutated in place.
    /// It returns the number of entries that were cleared by the rules.
    pub fn apply(&self, medias: &mut Vec<impl SourceEntry + NewNameMut>) -> usize {
        // this is just so that warnings are printed in a consistent order.
        medias.sort_unstable_by(|m, n| m.src_entry().cmp(n.src_entry()));

        // apply all rules in order.
        let total = medias.len();
        medias.retain_mut(|m| {
            let mut name = std::mem::take(m.new_name_mut());
            self.0.iter().for_each(|(re, to)| {
                if let Cow::Owned(x) = re.replace_all(&name, to) {
                    name = x;
                }
            });

            if name.is_empty() {
                eprintln!("blocked: rules cleared name: {}", m.src_entry());
                return false;
            }
            *m.new_name_mut() = name;
            true
        });
        total - medias.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entries::{Entry, ROOT};

    const NO_STRIP: [&[&str]; 3] = [&[], &[], &[]];
    const NO_REPLACE: &[(&str, &str)] = &[];
    const NO_DOWNGRADE: &[(&str, &str)] = &[];

    /// A dummy type that expects it is always changed.
    #[derive(Debug, PartialEq)]
    struct Media(String);
    impl NewNameMut for Media {
        fn new_name_mut(&mut self) -> &mut String {
            &mut self.0
        }
    }
    impl SourceEntry for Media {
        fn src_entry(&self) -> &Entry {
            &ROOT
        }
    }

    #[test]
    fn strip_rules() {
        #[track_caller]
        fn case(rule: &[&str], idx: usize, stem: &str, new_name: &str) {
            let mut strip_rules = [[].as_ref(); 3];
            strip_rules[idx] = rule;
            let mut medias = vec![Media(stem.to_owned())];
            let rules = NamingRules::compile(strip_rules, NO_REPLACE, NO_DOWNGRADE).unwrap();
            let warnings = rules.apply(&mut medias);
            assert_eq!(warnings, 0);
            assert_eq!(medias[0].0, new_name);
        }

        case(&["Before"], 0, "beforefoo", "foo");
        case(&["Before"], 0, "Before__foo", "__foo");
        case(&["Before"], 0, "before foo", "foo");
        case(&["before"], 0, "Before - foo", "foo");
        case(&["before"], 0, "before.foo", "foo");
        case(&["before"], 0, "Before\t.  foo", "foo");

        case(&["After"], 1, "fooafter", "foo");
        case(&["After"], 1, "foo__After", "foo__");
        case(&["After"], 1, "foo after", "foo");
        case(&["after"], 1, "foo - After", "foo");
        case(&["after"], 1, "foo.after", "foo");
        case(&["after"], 1, "foo\t. After", "foo");

        // exact: {BOUND}+{rule}$
        case(&["Exact"], 2, "foo__Exact", "foo__");
        case(&["Exact"], 2, "foo exact", "foo");
        case(&["exact"], 2, "foo - Exact", "foo");
        case(&["exact"], 2, "foo.exact", "foo");
        case(&["exact"], 2, "foo\t. Exact", "foo");

        // exact: ^{rule}{BOUND}+
        case(&["Exact"], 2, "Exact__foo", "__foo");
        case(&["Exact"], 2, "exact foo", "foo");
        case(&["exact"], 2, "Exact - foo", "foo");
        case(&["exact"], 2, "exact.foo", "foo");
        case(&["exact"], 2, "Exact\t.  foo", "foo");

        // exact: {BOUND}+{rule}
        case(&["Exact"], 2, "foo__Exactbar", "foo__bar");
        case(&["Exact"], 2, "foo exact bar", "foo bar");
        case(&["exact"], 2, "foo.exact.bar", "foo.bar");
        case(&["exact"], 2, "foo\t.  Exact - bar", "foo - bar");

        // exact: new boundaries
        case(&["exact"], 2, "foo - Exactbar", "foo - bar");
        case(&["Exact"], 2, "foo__Exact bar", "foo__ bar");
        case(&["Exact"], 2, "fooExact bar", "foo bar");
        case(&["(exact)"], 2, "foo - (Exact)bar", "foo - bar");
        case(&["(Exact)"], 2, "foo__(Exact) bar", "foo__ bar");
        case(&["Exact"], 2, "foo(Exact) bar", "foo bar");

        // exact: {rule}
        case(&["Exact"], 2, "fexactoo", "foo");
        case(&["Exact"], 2, "fexactoExacto", "foo");
        case(&["exact"], 2, "Exactfoo bar", "foo bar");
    }

    #[test]
    fn replace_rules() {
        #[track_caller]
        fn case(replace_rules: &[(&str, &str)], stem: &str, new_name: &str) {
            let mut medias = vec![Media(stem.to_owned())];
            let rules = NamingRules::compile(NO_STRIP, replace_rules, NO_DOWNGRADE).unwrap();
            let warnings = rules.apply(&mut medias);
            assert_eq!(warnings, 0);
            assert_eq!(medias[0].0, new_name);
        }

        case(&[("-+", "-")], "foo---bar", "foo-bar");
        case(&[(r"(\w+) +(\w+)", "$2 $1")], "foo  bar", "bar foo");
        case(&[(r"(.+)(S0\dE0\d)", "$2.$1")], "fooS03E05", "S03E05.foo");
    }

    #[test]
    fn downgrade_rules() {
        #[track_caller]
        fn case(downgrade_rules: &[(&str, &str)], stem: &str, new_name: &str) {
            let mut medias = vec![Media(stem.to_owned())];
            let rules = NamingRules::compile(NO_STRIP, NO_REPLACE, downgrade_rules).unwrap();
            let warnings = rules.apply(&mut medias);
            assert_eq!(warnings, 0);
            assert_eq!(medias[0].0, new_name);
        }

        case(
            &[("God.?of.?War", "God of War")],
            "other things",
            "other things",
        );
        case(
            &[("God.?of.?War", "God of War")],
            "God of War media",
            "media - God of War",
        );
        case(
            &[("God.?of.?War", "God of War")],
            "godofwar -  media",
            "media - God of War",
        );

        case(
            &[("God{S}of{S}War", "God of War")],
            "other things",
            "other things",
        );
        case(
            &[("God{S}of{S}War", "God of War")],
            "God of War media",
            "media - God of War",
        );
        case(
            &[("God{S}of{S}War", "God of War")],
            "godofwar media",
            "media - God of War",
        );
        case(
            &[("God{S}of{S}War", "God of War")],
            "God-of-War media",
            "media - God of War",
        );
        case(
            &[("God{S}of{S}War", "God of War")],
            "godofwar -  media",
            "media - God of War",
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
        let rules =
            NamingRules::compile([&["e"], &["b"], &["c.*i"]], &[("on", "")], NO_DOWNGRADE).unwrap();
        let warnings = rules.apply(&mut medias);
        assert_eq!(warnings, 4);
        assert_eq!(medias, vec![Media("foo".to_owned())]);
    }
}
