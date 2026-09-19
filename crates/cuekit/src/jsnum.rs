//! Reading numbers the way the JavaScript this was ported from reads them.
//!
//! The timestamp parser it came from calls `Number()` on the hours and the
//! minutes and `parseFloat()` on the seconds. The two disagree about almost
//! everything an ill-formed timestamp can contain — `Number("")` is zero
//! where `parseFloat("")` is not a number, `Number("1a")` is not a number
//! where `parseFloat("1a")` is one — and a subtitle file that has been cut,
//! concatenated or hand-edited produces exactly those. Reproducing both is
//! what makes a malformed file drop the same cues here as there.

/// What JavaScript's `Number(text)` gives.
///
/// Whitespace is trimmed, an empty string is zero, the radix prefixes are
/// honoured, and anything else that is not a complete numeric literal is not
/// a number. Note that `Number` accepts `Infinity` but not `NaN`, and neither
/// of the spellings Rust's own parser accepts for them.
pub fn number(text: &str) -> f64 {
    let text = text.trim_matches(is_js_space);
    if text.is_empty() {
        return 0.0;
    }
    if let Some(value) = radix_literal(text) {
        return value;
    }
    let body = text.strip_prefix(['+', '-']).unwrap_or(text);
    // Rust's parser accepts spellings JavaScript's does not.
    if body.eq_ignore_ascii_case("inf")
        || body.eq_ignore_ascii_case("nan")
        || body.eq_ignore_ascii_case("infinity")
    {
        return if body.eq_ignore_ascii_case("infinity") {
            match text.starts_with('-') {
                true => f64::NEG_INFINITY,
                false => f64::INFINITY,
            }
        } else {
            f64::NAN
        };
    }
    text.parse().unwrap_or(f64::NAN)
}

/// What JavaScript's `parseFloat(text)` gives: the longest decimal number at
/// the front, ignoring whatever follows, and not a number when there is none.
pub fn parse_float(text: &str) -> f64 {
    let text = text.trim_start_matches(is_js_space);
    let prefix = decimal_prefix(text);
    if prefix.is_empty() {
        return f64::NAN;
    }
    prefix.parse().unwrap_or(f64::NAN)
}

/// The longest leading run that is a decimal number, by JavaScript's grammar.
fn decimal_prefix(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut at = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        at = 1;
    }
    if text[at..].starts_with("Infinity") {
        return &text[..at + "Infinity".len()];
    }

    let mut end = at;
    let digits = |from: usize| {
        let mut to = from;
        while to < bytes.len() && bytes[to].is_ascii_digit() {
            to += 1;
        }
        to
    };
    end = digits(end);
    if end < bytes.len() && bytes[end] == b'.' {
        end = digits(end + 1);
    }
    // A run with no digits at all on either side of the point is not a
    // number: `.` and `+.` are nothing, `.5` and `5.` are numbers.
    if !text[at..end].chars().any(|c| c.is_ascii_digit()) {
        return "";
    }
    if end < bytes.len() && (bytes[end] | 0x20) == b'e' {
        let mut exponent = end + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        let after = digits(exponent);
        if after > exponent {
            end = after;
        }
    }
    &text[..end]
}

fn radix_literal(text: &str) -> Option<f64> {
    let (radix, digits) = match text.get(..2)?.to_ascii_lowercase().as_str() {
        "0x" => (16, &text[2..]),
        "0o" => (8, &text[2..]),
        "0b" => (2, &text[2..]),
        _ => return None,
    };
    if digits.is_empty() {
        return Some(f64::NAN);
    }
    Some(match u64::from_str_radix(digits, radix) {
        Ok(value) => value as f64,
        Err(_) => f64::NAN,
    })
}

/// JavaScript's whitespace for the purpose of trimming a numeric string:
/// Unicode whitespace plus the byte-order mark.
fn is_js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_reads_ordinary_numbers() {
        assert_eq!(number("12"), 12.0);
        assert_eq!(number("  07 "), 7.0);
        assert_eq!(number("1.5"), 1.5);
        assert_eq!(number("1e2"), 100.0);
        assert_eq!(number("-3"), -3.0);
    }

    #[test]
    fn number_of_nothing_is_zero() {
        assert_eq!(number(""), 0.0);
        assert_eq!(number("   "), 0.0);
    }

    #[test]
    fn number_refuses_a_partial_literal() {
        assert!(number("1a").is_nan());
        assert!(number("12:").is_nan());
        assert!(number("--1").is_nan());
    }

    #[test]
    fn number_does_not_accept_the_rust_spellings() {
        assert!(number("inf").is_nan());
        assert!(number("NaN").is_nan());
        assert_eq!(number("Infinity"), f64::INFINITY);
        assert_eq!(number("-Infinity"), f64::NEG_INFINITY);
    }

    #[test]
    fn number_honours_the_radix_prefixes() {
        assert_eq!(number("0x10"), 16.0);
        assert_eq!(number("0b101"), 5.0);
        assert_eq!(number("0o17"), 15.0);
    }

    #[test]
    fn parse_float_takes_the_longest_prefix() {
        assert_eq!(parse_float("23.456"), 23.456);
        assert_eq!(parse_float("23.456abc"), 23.456);
        assert_eq!(parse_float("  1.5xyz"), 1.5);
        assert_eq!(parse_float("5."), 5.0);
        assert_eq!(parse_float(".5"), 0.5);
        assert_eq!(parse_float("1e3x"), 1000.0);
        assert_eq!(parse_float("1e"), 1.0);
    }

    #[test]
    fn parse_float_of_nothing_is_not_a_number() {
        assert!(parse_float("").is_nan());
        assert!(parse_float("abc").is_nan());
        assert!(parse_float(".").is_nan());
        assert!(parse_float("+").is_nan());
    }
}
