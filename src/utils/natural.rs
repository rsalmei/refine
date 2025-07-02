use std::cmp::Ordering;
use std::iter::Peekable;
use std::str::Chars;

/// Compare two strings in a natural order, case-insensitive.
pub fn natural_cmp(a: impl AsRef<str>, b: impl AsRef<str>) -> Ordering {
    let mut a_chars = a.as_ref().chars().peekable();
    let mut b_chars = b.as_ref().chars().peekable();

    while let Some(a_peek) = a_chars.peek()
        && let Some(b_peek) = b_chars.peek()
    {
        let a_is_digit = a_peek.is_ascii_digit();
        let b_is_digit = b_peek.is_ascii_digit();

        let ordering = match (a_is_digit, b_is_digit) {
            (true, true) => compare_num_chunks(&mut a_chars, &mut b_chars),
            (false, false) => compare_text_chunks(&mut a_chars, &mut b_chars),
            (true, false) => Ordering::Less, // numbers come before text.
            (false, true) => Ordering::Greater, // text comes after numbers.
        };

        if ordering != Ordering::Equal {
            return ordering;
        }
    }

    // check for remaining characters in either string.
    a_chars.peek().is_some().cmp(&b_chars.peek().is_some())
}

/// Compare numeric chunks directly from the character iterator.
fn compare_num_chunks(a_chars: &mut Peekable<Chars>, b_chars: &mut Peekable<Chars>) -> Ordering {
    fn parse_number(chars: &mut Peekable<Chars>) -> (u64, usize) {
        let (mut value, mut length) = (0u64, 0);

        while let Some(&c) = chars.peek()
            && c.is_ascii_digit()
        {
            let digit = chars.next().unwrap(); // just peeked.
            value = value
                .saturating_mul(10) // saturating to prevent overflow for very large numbers.
                .saturating_add((digit as u32 - '0' as u32) as u64);
            length += 1;
        }

        (value, length)
    }

    let (num_a, len_a) = parse_number(a_chars);
    let (num_b, len_b) = parse_number(b_chars);

    // compare numeric values first, then original length for leading zeros.
    num_a.cmp(&num_b).then_with(|| len_a.cmp(&len_b))
}

/// Compare text chunks case-insensitively directly from the character iterators.
fn compare_text_chunks(a_chars: &mut Peekable<Chars>, b_chars: &mut Peekable<Chars>) -> Ordering {
    fn consume_remaining_text(chars: &mut Peekable<Chars>) {
        while let Some(&c) = chars.peek()
            && !c.is_ascii_digit()
        {
            chars.next(); // consume remaining text characters.
        }
    }

    while let Some(a_peek) = a_chars.peek()
        && !a_peek.is_ascii_digit()
        && let Some(b_peek) = b_chars.peek()
        && !b_peek.is_ascii_digit()
    {
        let (a, b) = (a_chars.next().unwrap(), b_chars.next().unwrap()); // just peeked.
        let ordering = a.to_lowercase().cmp(b.to_lowercase());
        if ordering != Ordering::Equal {
            // consume remaining text characters from both iterators before returning.
            consume_remaining_text(a_chars);
            consume_remaining_text(b_chars);
            return ordering;
        }
    }

    // peek at both iterators to check for digits or end.
    match (a_chars.peek(), b_chars.peek()) {
        // a still has text, b has digit or end.
        (Some(a_char), _) if !a_char.is_ascii_digit() => {
            consume_remaining_text(a_chars);
            Ordering::Greater
        }
        // b still has text, a has digit or end.
        (_, Some(b_char)) if !b_char.is_ascii_digit() => {
            consume_remaining_text(b_chars);
            Ordering::Less
        }
        // both have digits or both are at end.
        _ => Ordering::Equal,
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
