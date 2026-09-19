//! Turning HTML character references back into the characters they name.
//!
//! Converted markdown carries doubly-escaped markup: a tag that was escaped
//! once by the source and once by the converter arrives as `&lt;sup>107`,
//! which no tag pattern can see. Decoding first turns it into a real tag that
//! the stripping pass then removes, and fixes `&amp;` and the numeric forms
//! in ordinary prose on the way past.
//!
//! The rules are the specification's, including the parts that exist only
//! because browsers had to accept what was already on the web: a reference
//! with no closing semicolon, a name that is the prefix of a longer name, and
//! numbers that mean Windows-1252 positions rather than code points.

use std::sync::LazyLock;

use regex::Regex;

use crate::entities::{INVALID_CHARREFS, INVALID_CODEPOINTS, NAMED};

/// A character reference: a number, a hexadecimal number, or a name of up to
/// thirty-two characters, each with an optional closing semicolon.
static CHARREF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"&(#[0-9]+;?|#[xX][0-9a-fA-F]+;?|[^\t\n\x0c <&#;]{1,32};?)").expect("valid")
});

/// Decode every character reference in `text`.
pub fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    CHARREF
        .replace_all(text, |found: &regex::Captures| replace(&found[1]))
        .into_owned()
}

fn replace(reference: &str) -> String {
    match reference.strip_prefix('#') {
        Some(number) => numeric(number),
        None => named(reference),
    }
}

fn numeric(number: &str) -> String {
    let digits = number.trim_end_matches(';');
    let value = match digits.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => digits.parse::<u32>(),
    };
    // A number too long to be a code point at all is out of range, which the
    // specification answers the same way as any other out-of-range number.
    let Ok(value) = value else {
        return '\u{fffd}'.to_string();
    };

    if let Ok(at) = INVALID_CHARREFS.binary_search_by_key(&value, |(number, _)| *number) {
        return INVALID_CHARREFS[at].1.to_string();
    }
    if (0xd800..=0xdfff).contains(&value) || value > 0x10ffff {
        return '\u{fffd}'.to_string();
    }
    if INVALID_CODEPOINTS.binary_search(&value).is_ok() {
        return String::new();
    }
    char::from_u32(value).map(String::from).unwrap_or_default()
}

fn named(name: &str) -> String {
    if let Some(value) = lookup(name) {
        return value.to_string();
    }
    // The longest name that is a prefix of what was written wins, and the
    // rest is left as text: `&notit;` is `¬it;`, not an unknown reference.
    for end in (2..name.len()).rev() {
        if !name.is_char_boundary(end) {
            continue;
        }
        if let Some(value) = lookup(&name[..end]) {
            return format!("{value}{}", &name[end..]);
        }
    }
    format!("&{name}")
}

fn lookup(name: &str) -> Option<&'static str> {
    NAMED
        .binary_search_by_key(&name, |(named, _)| named)
        .ok()
        .map(|at| NAMED[at].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_so_it_can_be_searched() {
        assert!(NAMED.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(
            INVALID_CHARREFS
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0)
        );
        assert!(INVALID_CODEPOINTS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn ordinary_references_decode() {
        assert_eq!(unescape("a &amp; b"), "a & b");
        assert_eq!(unescape("&lt;sup&gt;107"), "<sup>107");
        assert_eq!(unescape("&#65;&#x42;"), "AB");
    }

    #[test]
    fn a_missing_semicolon_is_still_a_reference() {
        assert_eq!(unescape("&amp"), "&");
        assert_eq!(unescape("&#65"), "A");
        assert_eq!(unescape("&AElig"), "Æ");
    }

    #[test]
    fn the_longest_name_wins_and_the_rest_stays_text() {
        assert_eq!(unescape("&notit;"), "¬it;");
        assert_eq!(unescape("&notin;"), "∉");
    }

    #[test]
    fn something_that_is_not_a_reference_is_left_alone() {
        assert_eq!(unescape("&unknown;"), "&unknown;");
        assert_eq!(unescape("a & b"), "a & b");
        assert_eq!(unescape("no ampersand here"), "no ampersand here");
    }

    #[test]
    fn windows_1252_numbers_decode_as_the_specification_says() {
        assert_eq!(unescape("&#128;"), "€");
        assert_eq!(unescape("&#0;"), "\u{fffd}");
    }

    #[test]
    fn a_number_that_is_not_a_code_point_is_refused() {
        assert_eq!(unescape("&#xD800;"), "\u{fffd}");
        assert_eq!(unescape("&#1114112;"), "\u{fffd}");
        assert_eq!(unescape("&#99999999999999999999;"), "\u{fffd}");
    }

    #[test]
    fn a_code_point_the_specification_drops_comes_out_as_nothing() {
        assert_eq!(unescape("a&#1;b"), "ab");
    }
}
