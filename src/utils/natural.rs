use regex::Regex;
use std::cmp::Ordering;
use std::sync::LazyLock;

static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)|(\D+)").unwrap());

/// Compare two strings in a natural order, case-insensitive.
pub fn natural_cmp(a: impl AsRef<str>, b: impl AsRef<str>) -> Ordering {
    let chunks = |x| {
        RE.captures_iter(x).map(|cap| {
            if let Some(m) = cap.get(1) {
                (true, m.as_str()) // numeric chunk (still as str, as it might not be necessary to parse).
            } else {
                (false, cap.get(2).unwrap().as_str()) // text chunk, guaranteed.
            }
        })
    };
    let mut a_it = chunks(a.as_ref());
    let mut b_it = chunks(b.as_ref());

    for ((a_is_num, a_val), (b_is_num, b_val)) in a_it.by_ref().zip(b_it.by_ref()) {
        match (a_is_num, b_is_num) {
            // both chunks are numeric.
            (true, true) => {
                let num_a = a_val.parse::<u64>().unwrap_or_default(); // regex guarantees they're parsable,
                let num_b = b_val.parse::<u64>().unwrap_or_default(); // but they might not fit an u64...
                match num_a.cmp(&num_b) {
                    Ordering::Equal => match a_val.len().cmp(&b_val.len()) {
                        Ordering::Equal => {}
                        res => return res,
                    },
                    res => return res,
                }
            }
            // both chunks are text.
            (false, false) => {
                let a_it = a_val.chars().flat_map(|x| x.to_lowercase());
                let b_it = b_val.chars().flat_map(|x| x.to_lowercase());
                let res = a_it.cmp(b_it);
                if res != Ordering::Equal {
                    return res;
                }
            }
            // numeric chunks come before text chunks.
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
        }
    }

    // if all compared chunks were equal, check remaining.
    match (a_it.next(), b_it.next()) {
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
        (Some(_), Some(_)) => unreachable!(), // zip guarantees that one of them is exhausted.
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        let mut values = vec!["2", "10", "1"];
        values.sort_unstable_by(|a, b| natural_cmp(a, b));
        assert_eq!(values, ["1", "2", "10"]);
    }

    #[test]
    fn text() {
        let mut values = vec!["b", "c", "a"];
        values.sort_unstable_by(|a, b| natural_cmp(a, b));
        assert_eq!(values, ["a", "b", "c"]);
    }

    #[test]
    fn mixed() {
        let mut values = vec!["file1", "file10", "file2"];
        values.sort_unstable_by(|a, b| natural_cmp(a, b));
        assert_eq!(values, ["file1", "file2", "file10"]);
    }

    #[test]
    fn mixed_suffix() {
        let mut values = vec!["file1", "file10", "file2"];
        values.sort_unstable_by(|a, b| natural_cmp(a, b));
        assert_eq!(values, ["file1", "file2", "file10"]);
    }

    #[test]
    fn complex() {
        let mut values = vec!["file1", "file1B", "file00", "file11", "file0002"];
        values.sort_unstable_by(|a, b| natural_cmp(a, b));
        assert_eq!(values, ["file00", "file1", "file1B", "file0002", "file11"]);
    }

    #[test]
    fn hierarchical() {
        let mut values = vec!["file-10", "file-1", "file-1-2", "file-2", "file-1-10"];
        values.sort_unstable_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            values,
            ["file-1", "file-1-2", "file-1-10", "file-2", "file-10",]
        );
    }

    #[test]
    fn with_zeros() {
        let mut values = vec!["file01", "file1", "file10", "file001"];
        values.sort_unstable_by(|a, b| natural_cmp(a, b));
        assert_eq!(values, ["file1", "file01", "file001", "file10"]);
    }

    #[test]
    fn empty_strings() {
        let mut values = vec!["", "file1", ""];
        values.sort_unstable_by(|a, b| natural_cmp(a, b));
        assert_eq!(values, ["", "", "file1"]);
    }

    #[test]
    fn actual_strings() {
        let mut values = vec!["file2".to_string(), "file10".to_string()];
        values.sort_unstable_by(|a, b| natural_cmp(a, b));
        assert_eq!(values, ["file2", "file10"]);
    }
}
