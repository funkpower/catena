//! Finding and checking ISBNs.

use std::sync::LazyLock;

use regex::Regex;
use textkit::pattern::SPACE;

/// An ISBN announced as one, in running text or a catalogue field.
static ANNOUNCED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)ISBN[-{SPACE}:]*((?:97[89][{SPACE}-]?)?(?:[0-9][{SPACE}-]?){{9}}[0-9Xx])"
    ))
    .expect("a valid pattern")
});

/// A bare ISBN sitting in a filename.
static BARE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(97[89][0-9]{10}|[0-9]{9}[0-9Xx])").expect("a valid pattern"));

/// The extension at the end of a filename.
static EXTENSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.[a-z0-9]+$").expect("a valid pattern"));

/// Whether this is a real ISBN, by its check digit.
///
/// The checksum is the point. Without it any thirteen digits in a filename —
/// a scan date, an internal identifier, a phone number — reads as an ISBN,
/// and an ISBN is what a catalogue lookup trusts absolutely.
pub fn valid(text: &str) -> bool {
    let digits: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x')
        .collect();

    match digits.len() {
        13 if digits.chars().all(|c| c.is_ascii_digit()) => {
            let sum: u32 = digits
                .chars()
                .enumerate()
                .map(|(at, digit)| {
                    let weight = if at % 2 == 0 { 1 } else { 3 };
                    weight * digit.to_digit(10).expect("a digit")
                })
                .sum();
            sum % 10 == 0
        }
        10 if digits[..9].chars().all(|c| c.is_ascii_digit()) => {
            let sum: u32 = digits
                .chars()
                .enumerate()
                .map(|(at, c)| {
                    let value = if c == 'X' || c == 'x' {
                        10
                    } else {
                        c.to_digit(10).unwrap_or(0)
                    };
                    (10 - at as u32) * value
                })
                .sum();
            sum % 11 == 0
        }
        _ => false,
    }
}

/// Just the digits of an ISBN, with the spacing and hyphens taken out.
pub fn digits(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x')
        .collect()
}

/// An ISBN announced in a piece of text, if it is the right length.
pub fn from_text(text: &str) -> Option<String> {
    let found = ANNOUNCED.captures(text)?;
    let digits = digits(&found[1]);
    matches!(digits.len(), 10 | 13).then_some(digits)
}

/// A checksum-valid ISBN embedded in a filename.
///
/// A file is often named exactly its ISBN and nothing else, which makes a
/// free and certain identifier out of a record that otherwise has none.
pub fn from_filename(name: &str) -> Option<String> {
    let stem = EXTENSION.replace(name, "");
    BARE.find_iter(&stem)
        .map(|found| found.as_str())
        .find(|candidate| valid(candidate))
        .map(digits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_isbn_checks_out() {
        assert!(valid("9780198237907"));
        assert!(valid("978-0-19-823790-7"));
        assert!(valid("0198237901"));
        assert!(valid("080213825X"));
    }

    #[test]
    fn a_number_that_is_merely_the_right_length_does_not() {
        assert!(!valid("9780198237908"));
        assert!(!valid("0198237902"));
        assert!(!valid("1234567890123"));
        assert!(!valid(""));
        assert!(!valid("not a number"));
    }

    #[test]
    fn an_isbn_is_found_where_it_says_it_is() {
        assert_eq!(
            from_text("ISBN: 978-0-19-823790-7"),
            Some("9780198237907".to_string())
        );
        assert_eq!(
            from_text("isbn 0198237901 (paperback)"),
            Some("0198237901".to_string())
        );
        assert_eq!(from_text("no identifier here"), None);
    }

    #[test]
    fn a_filename_that_is_an_isbn_gives_it_up() {
        assert_eq!(
            from_filename("9780198237907.pdf"),
            Some("9780198237907".to_string())
        );
        assert_eq!(
            from_filename("Aquinas - 9780198237907 (scan).epub"),
            Some("9780198237907".to_string())
        );
    }

    #[test]
    fn a_filename_of_digits_that_is_not_an_isbn_gives_nothing() {
        assert_eq!(from_filename("20231115123456.pdf"), None);
        assert_eq!(from_filename("chapter one.pdf"), None);
    }

    #[test]
    fn the_extension_is_not_searched() {
        // Otherwise a name could borrow digits from its own suffix.
        assert_eq!(from_filename("book.9780198237907"), None);
    }
}
