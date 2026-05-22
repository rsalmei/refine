use crate::entries::Entry;
use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

/// Get the stem and extension from files, or name from directories.
pub fn filename_parts(entry: &Entry) -> (&str, &'static str) {
    match entry.is_dir {
        true => (entry.file_name(), ""),
        false => {
            let ext = intern(entry.path.extension().unwrap_or_default().to_str().unwrap());
            (entry.path.file_stem().unwrap().to_str().unwrap(), ext)
        }
    }
}

fn intern(text: &str) -> &'static str {
    static CACHE: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);

    let mut cache = CACHE.lock().unwrap();
    match cache.get(text) {
        Some(x) => x,
        None => {
            let interned = Box::leak(text.to_owned().into_boxed_str());
            cache.insert(interned);
            interned
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}
