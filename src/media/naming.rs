use super::{NewNameMut, SourceEntry};
use crate::utils;
use anyhow::{Context, Result};
use clap::Args;
use clap::builder::NonEmptyStringValueParser;
use regex::Regex;
use std::borrow::Cow;

/// A set of rules that allows the user to customize filenames.
#[derive(Debug, Args)]
pub struct Naming {
    /// Strip from the start till occurrence; includes separators nearby, use {S} for separators.
    #[arg(short = 'b', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new()
    )]
    strip_before: Vec<String>,
    /// Strip from occurrence till the end; includes separators nearby, use {S} for separators.
    #[arg(short = 'a', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new()
    )]
    strip_after: Vec<String>,
    /// Strip exact occurrences; includes separators nearby, use {S} for separators.
    #[arg(short = 'e', long, value_name = "STR|REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new()
    )]
    strip_exact: Vec<String>,
    /// Replace occurrences in the filename; separators are not touched, use {S} for separators.
    #[arg(short = 'r', long, value_name = "{STR|REGEX}={STR|$N}", allow_hyphen_values = true, value_parser = utils::parse_key_value::<String, String>
    )]
    replace: Vec<(String, String)>,
    /// recipe: Throw some prefix to the end; use {S} for separators.
    #[arg(short = 'w', long, value_name = "{STR|REGEX}={STR|$N}", allow_hyphen_values = true, value_parser = utils::parse_key_value::<String, String>
    )]
    throw: Vec<(String, String)>,
}

impl Naming {
    /// Compile this set of rules.
    pub fn compile(&self) -> Result<NamingRules> {
        NamingRules::compile(
            &self.strip_before,
            &self.strip_after,
            &self.strip_exact,
            &self.replace,
            &self.throw,
        )
    }
}

#[derive(Debug)]
pub struct NamingRules(Vec<(Regex, String)>);

impl NamingRules {
    fn compile(
        strip_before_rules: &[impl AsRef<str>],
        strip_after_rules: &[impl AsRef<str>],
        strip_exact_rules: &[impl AsRef<str>],
        replace_rules: &[(impl AsRef<str>, impl AsRef<str>)],
        throw_rules: &[(impl AsRef<str>, impl AsRef<str>)],
    ) -> Result<NamingRules> {
        const O: &str = r"[(\[{]"; // enclosing opening.
        const C: &str = r"[)\]}]"; // enclosing closing.
        const SEP: &str = r"[-\s.,]";
        let before = |rule| format!("^.*{rule}{C}*{SEP}*");
        let after = |rule| format!("{SEP}*{O}*{rule}.*$");
        let exact = |rule| {
            let core = format!(r"{O}*{rule}{C}*");
            format!(r"^{core}{SEP}+|{SEP}+{core}$|{SEP}+{core}(?P<kept_sep>{SEP}+)|{core}")
        };
        let replace_key = |rule: &str| rule.to_owned();
        let throw_key = |rule| format!(r"^{rule}{SEP}+(?<rest>.+)$");
        let throw_value = |val| format!(r"$rest - {val}");

        let rules = strip_before_rules
            .iter()
            .map(|r| r.as_ref())
            .map(|rule| (rule, before(rule), String::new()))
            .chain(
                strip_after_rules
                    .iter()
                    .map(|r| r.as_ref())
                    .map(|rule| (rule, after(rule), String::new())),
            )
            .chain(
                strip_exact_rules
                    .iter()
                    .map(|r| r.as_ref())
                    .map(|rule| (rule, exact(rule), "$kept_sep".to_owned())),
            )
            .chain(
                replace_rules
                    .iter()
                    .map(|(k, v)| (k.as_ref(), v.as_ref()))
                    .map(|(rule, to)| (rule, replace_key(rule), to.to_owned())),
            )
            .chain(
                throw_rules
                    .iter()
                    .map(|(k, v)| (k.as_ref(), v.as_ref()))
                    .map(|(rule, to)| (rule, throw_key(rule), throw_value(to))),
            )
            .map(|(rule, key, to)| {
                Regex::new(&format!("(?i){}", key.replace("{S}", SEP))) // support {S} for separators.
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
                eprintln!("ignored: rules cleared name: {}", m.src_entry());
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

    const NO_STRIP: &[&str] = &[];
    const NO_REPLACE: &[(&str, &str)] = &[];
    const NO_THROW: &[(&str, &str)] = &[];

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
        fn case(before: &[&str], after: &[&str], exact: &[&str], stem: &str, new_name: &str) {
            let mut medias = vec![Media(stem.to_owned())];
            let rules = NamingRules::compile(before, after, exact, NO_REPLACE, NO_THROW).unwrap();
            let warnings = rules.apply(&mut medias);
            assert_eq!(warnings, 0);
            assert_eq!(medias[0].0, new_name);
        }

        case(&["Before"], &[], &[], "beforefoo", "foo");
        case(&["Before"], &[], &[], "Before__foo", "__foo");
        case(&["Before"], &[], &[], "before foo", "foo");
        case(&["before"], &[], &[], "Before - foo", "foo");
        case(&["before"], &[], &[], "before.foo", "foo");
        case(&["before"], &[], &[], "Before\t.  foo", "foo");

        case(&[], &["After"], &[], "fooafter", "foo");
        case(&[], &["After"], &[], "foo__After", "foo__");
        case(&[], &["After"], &[], "foo after", "foo");
        case(&[], &["after"], &[], "foo - After", "foo");
        case(&[], &["after"], &[], "foo.after", "foo");
        case(&[], &["after"], &[], "foo\t. After", "foo");

        // exact: ^{rule}{BOUND}+
        case(&[], &[], &["Exact"], "Exact__foo", "__foo");
        case(&[], &[], &["Exact"], "exact foo", "foo");
        case(&[], &[], &["exact"], "Exact - foo", "foo");
        case(&[], &[], &["exact"], "exact.foo", "foo");
        case(&[], &[], &["exact"], "Exact\t.  foo", "foo");

        // exact: {BOUND}+{rule}$
        case(&[], &[], &["Exact"], "foo__Exact", "foo__");
        case(&[], &[], &["Exact"], "foo exact", "foo");
        case(&[], &[], &["exact"], "foo - Exact", "foo");
        case(&[], &[], &["exact"], "foo.exact", "foo");
        case(&[], &[], &["exact"], "foo\t. Exact", "foo");

        // exact: {BOUND}+{rule}
        case(&[], &[], &["Exact"], "foo__Exactbar", "foo__bar");
        case(&[], &[], &["Exact"], "foo exact bar", "foo bar");
        case(&[], &[], &["exact"], "foo.exact.bar", "foo.bar");
        case(&[], &[], &["exact"], "foo\t.  Exact - bar", "foo - bar");

        // exact: new boundaries
        case(&[], &[], &["exact"], "foo - Exactbar", "foo - bar");
        case(&[], &[], &["Exact"], "foo__Exact bar", "foo__ bar");
        case(&[], &[], &["Exact"], "fooExact bar", "foo bar");
        case(&[], &[], &["exact"], "foo - (Exact)bar", "foo - bar");
        case(&[], &[], &["Exact"], "foo__(Exact) bar", "foo__ bar");
        case(&[], &[], &["Exact"], "foo(Exact) bar", "foo bar");

        // exact: {rule}
        case(&[], &[], &["Exact"], "fexactoo", "foo");
        case(&[], &[], &["Exact"], "fexactoExacto", "foo");
        case(&[], &[], &["exact"], "Exactfoo bar", "foo bar");
    }

    #[test]
    fn replace_rules() {
        #[track_caller]
        fn case(replace_rules: &[(&str, &str)], stem: &str, new_name: &str) {
            let mut medias = vec![Media(stem.to_owned())];
            let rules = NamingRules::compile(NO_STRIP, NO_STRIP, NO_STRIP, replace_rules, NO_THROW)
                .unwrap();
            let warnings = rules.apply(&mut medias);
            assert_eq!(warnings, 0);
            assert_eq!(medias[0].0, new_name);
        }

        case(&[("-+", "-")], "foo---bar", "foo-bar");
        case(&[(r"(\w+) +(\w+)", "$2 $1")], "foo  bar", "bar foo");
        case(&[(r"(.+)(S0\dE0\d)", "$2.$1")], "fooS03E05", "S03E05.foo");
    }

    #[test]
    fn throw_rules() {
        #[track_caller]
        fn cases(throw_rules: &[(&str, &str)], stem_names: &[(&str, &str)]) {
            let rules = NamingRules::compile(NO_STRIP, NO_STRIP, NO_STRIP, NO_REPLACE, throw_rules)
                .unwrap();
            for &(stem, new_name) in stem_names {
                let mut medias = vec![Media(stem.to_owned())];
                let warnings = rules.apply(&mut medias);
                assert_eq!(warnings, 0);
                assert_eq!(medias[0].0, new_name);
            }
        }

        cases(
            &[("God.?of.?War", "God of War")],
            &[
                ("other things", "other things"),
                ("God of War media", "media - God of War"),
                ("godofwar -  media", "media - God of War"),
            ],
        );
        cases(
            &[("God{S}*of{S}*War", "God of War")],
            &[
                ("godofwar media", "media - God of War"),
                ("godofwar -  media", "media - God of War"),
                ("god of war -  media", "media - God of War"),
                ("God-of-War  media", "media - God of War"),
                ("God   of    War files", "files - God of War"),
            ],
        );
        cases(
            &[(r"God\s*of\s*(\w+)", "God of $1")],
            &[
                ("godofwar media", "media - God of war"),
                ("god of  zilla - media", "media - God of zilla"),
                ("GOD   OF   THUNDER files", "files - God of THUNDER"),
            ],
        )
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
            NamingRules::compile(&["e"], &["b"], &["c.*i"], &[("on", "")], NO_THROW).unwrap();
        let warnings = rules.apply(&mut medias);
        assert_eq!(warnings, 4);
        assert_eq!(medias, vec![Media("foo".to_owned())]);
    }
}
