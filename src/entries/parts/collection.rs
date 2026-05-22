use regex::Regex;
use std::sync::LazyLock;

/// Get the canonical name, alias, sequence, comment, and extension from collections.
pub fn collection_parts(stem: &str) -> (&str, Option<&str>, Option<usize>, &str) {
    /// Regex for well-formed collection names, already rebuilt into the standard format. It
    /// captures the name, optional alias, sequence number, and optional comment.
    /// ex: name~24 or name+alias~24.
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(\w+)(?:\+(\w+))?~(\d+)[ -]*(.*)$").unwrap());

    let Some(caps) = RE.captures(stem) else {
        /// Fallback regex for names that don't match the standard format, capturing a name and
        /// comment only. This is more lenient and allows for various formats, for new filenames
        /// that haven't been rebuilt yet.
        /// ex: "name comment" or "name - comment".
        static FALLBACK_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^(\w+)[ -]+(.*)$").unwrap());
        let Some(caps) = FALLBACK_RE.captures(stem) else {
            return (stem, None, None, "");
        };
        let name = caps.get(1).unwrap().as_str(); // SAFETY: regex guarantees group 1 (name) is present.
        let comment = caps.get(2).map_or("", |m| m.as_str());
        return (name, None, None, comment);
    };
    let name = caps.get(1).unwrap().as_str(); // SAFETY: regex guarantees group 1 (name) is present.
    let alias = caps.get(2).map(|m| m.as_str());
    let seq = caps.get(3).and_then(|m| m.as_str().parse().ok());
    let comment = caps.get(4).map_or("", |m| m.as_str());
    (name, alias, seq, comment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fn_collection_parts() {
        #[track_caller]
        fn case(
            stem: &str,
            (name, alias, seq, comment): (&str, Option<&str>, Option<usize>, &str),
        ) {
            assert_eq!(collection_parts(stem), (name, alias, seq, comment));
        }

        // stem only.
        case("foo", ("foo", None, None, ""));
        case("foo~ 24", ("foo~ 24", None, None, ""));
        case("foo+bar", ("foo+bar", None, None, ""));
        case("foo+bar,baz", ("foo+bar,baz", None, None, ""));
        case("foo+bar ~ 24", ("foo+bar ~ 24", None, None, ""));
        case("foo+asd ~24", ("foo+asd ~24", None, None, ""));
        case("foo+~24", ("foo+~24", None, None, ""));
        case(",~24", (",~24", None, None, ""));
        case("foo+ ~24", ("foo+ ~24", None, None, ""));
        case("foo+,~24", ("foo+,~24", None, None, ""));
        case("foo+bar,~24", ("foo+bar,~24", None, None, ""));
        case("foo+ asd~24", ("foo+ asd~24", None, None, ""));
        case("foo+bar,~24 cool", ("foo+bar,~24 cool", None, None, ""));

        // name and comment.
        case("_foo_-24", ("_foo_", None, None, "24"));
        case("foo bar", ("foo", None, None, "bar"));
        case("foo bar - baz", ("foo", None, None, "bar - baz"));
        case("_foo_ -24", ("_foo_", None, None, "24"));
        case("foo - 2025 - 24", ("foo", None, None, "2025 - 24"));
        case("foo ~24", ("foo", None, None, "~24"));
        case("foo bar~24", ("foo", None, None, "bar~24"));
        case("foo bar ~24", ("foo", None, None, "bar ~24"));
        case("_foo_ ~24", ("_foo_", None, None, "~24"));
        case("foo - 33~24", ("foo", None, None, "33~24"));
        case("foo ~ 24", ("foo", None, None, "~ 24"));

        // name and seq.
        case("foo~24", ("foo", None, Some(24), ""));
        case("foo_~24", ("foo_", None, Some(24), ""));
        case("__foo~24", ("__foo", None, Some(24), ""));
        case("_foo__~24", ("_foo__", None, Some(24), ""));

        // name, aliases and seq.
        case("foo+bar~24", ("foo", Some("bar"), Some(24), ""));
        case(
            "foo_bar__+_baz__~24",
            ("foo_bar__", Some("_baz__"), Some(24), ""),
        );

        // name, seq, and comment.
        case("foo~24cool", ("foo", None, Some(24), "cool"));
        case("foo~24 cool", ("foo", None, Some(24), "cool"));
        case("foo_~24-nice!", ("foo_", None, Some(24), "nice!"));
        case("__foo~24 ?why?", ("__foo", None, Some(24), "?why?"));
        case("_foo__~24 - cut", ("_foo__", None, Some(24), "cut"));
        case("_foo__~24 42", ("_foo__", None, Some(24), "42"));

        // name, aliases, seq, and comment.
        case(
            "foo+bar~24 seen 3 times",
            ("foo", Some("bar"), Some(24), "seen 3 times"),
        );
        case(
            "_foo+__bar_~24 with comment!",
            ("_foo", Some("__bar_"), Some(24), "with comment!"),
        );
    }
}
