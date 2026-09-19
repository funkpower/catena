//! International standard serial numbers, which identify a journal.
//!
//! Eight digits in two groups of four, the last of which is a check digit
//! that may be `X` for ten.

use std::sync::LazyLock;

use fancy_regex::Regex;

/// Eight characters in two fours, with or without the hyphen, and with
/// whatever introduced it.
static ISSN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\d{4})-?(\d{3}[\dX])\b").expect("a valid pattern"));

/// Read an ISSN out of a string, hyphenated as it should be stored.
pub fn parse(text: &str) -> Option<String> {
    let text = text.trim();
    let without = text
        .strip_prefix("ISSN ")
        .or_else(|| text.strip_prefix("issn "))
        .or_else(|| text.strip_prefix("ISSN:"))
        .or_else(|| text.strip_prefix("issn:"))
        .unwrap_or(text)
        .trim();

    let found = ISSN.captures(without).ok().flatten()?;
    Some(format!(
        "{}-{}",
        found.get(1)?.as_str(),
        found.get(2)?.as_str().to_uppercase()
    ))
}

/// Whether the check digit agrees with the other seven.
///
/// Each of the first seven digits is weighted by its position counting down
/// from eight, and the total plus the check digit must be a multiple of
/// eleven. One mistyped digit, or two transposed, fails.
pub fn valid(issn: &str) -> bool {
    let digits: Vec<char> = issn
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    if digits.len() != 8 {
        return false;
    }

    let mut total = 0u32;
    for (index, digit) in digits[..7].iter().enumerate() {
        let Some(value) = digit.to_digit(10) else {
            return false;
        };
        total += value * (8 - index as u32);
    }
    let check = match digits[7] {
        'X' | 'x' => 10,
        digit => match digit.to_digit(10) {
            Some(value) => value,
            None => return false,
        },
    };
    (total + check) % 11 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_issn_checks_out() {
        // Nature.
        assert!(valid("0028-0836"));
        assert!(valid("00280836"));
        // Science.
        assert!(valid("0036-8075"));
    }

    #[test]
    fn one_digit_out_does_not() {
        assert!(!valid("0028-0837"));
        assert!(!valid("0028-0835"));
    }

    #[test]
    fn two_digits_swapped_does_not_either() {
        // Which is the whole point of a weighted checksum.
        assert!(!valid("0082-0836"));
    }

    #[test]
    fn a_check_digit_of_ten_is_written_x() {
        assert!(valid("1050-124X"));
        assert!(valid("1050-124x"), "however it is cased");
    }

    #[test]
    fn what_is_the_wrong_length_or_shape_is_not_valid() {
        assert!(!valid(""));
        assert!(!valid("0028-083"));
        assert!(!valid("0028-08366"));
        assert!(!valid("0028-083Y"));
        assert!(!valid("ABCD-EFGH"));
    }

    #[test]
    fn it_is_read_however_it_was_written() {
        for text in [
            "0028-0836",
            "00280836",
            "ISSN 0028-0836",
            "ISSN: 0028-0836",
            " 0028-0836 ",
        ] {
            assert_eq!(parse(text).as_deref(), Some("0028-0836"), "{text}");
        }
    }

    #[test]
    fn a_lower_case_check_digit_is_stored_upper() {
        assert_eq!(parse("1050-124x").as_deref(), Some("1050-124X"));
    }

    #[test]
    fn what_is_not_an_issn_is_not_one() {
        for text in ["", "hello", "123", "0028-08AB"] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }
}
