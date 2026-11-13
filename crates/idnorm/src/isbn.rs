//! An ISBN's two forms, ten digits and thirteen.
//!
//! A book printed before 2007 carries a ten-digit ISBN, one printed since
//! carries thirteen, and every ten-digit one has a thirteen-digit twin: `978`
//! in front and the check digit worked out again. A catalogue may file a
//! book under either, so asking about one form and not the other misses
//! books that are there.
//!
//! Only a well-formed ISBN is converted. Working out a fresh check digit for
//! one whose own check digit is wrong would not correct the typing error, it
//! would turn the number into a different book's.

/// Just the digits of an ISBN, with a check character of ten as `X`.
pub fn digits(text: &str) -> String {
    bibclean::isbn::digits(text).to_ascii_uppercase()
}

/// The thirteen-digit form of an ISBN, from either form.
///
/// `None` when the text is not an ISBN whose check digit agrees.
pub fn to_13(text: &str) -> Option<String> {
    let digits = digits(text);
    if !bibclean::isbn::valid(&digits) {
        return None;
    }
    match digits.len() {
        13 => Some(digits),
        10 => {
            let body = format!("978{}", &digits[..9]);
            let sum: u32 = body
                .bytes()
                .enumerate()
                .map(|(at, b)| u32::from(b - b'0') * if at % 2 == 0 { 1 } else { 3 })
                .sum();
            Some(format!("{body}{}", (10 - sum % 10) % 10))
        }
        _ => None,
    }
}

/// The ten-digit form of an ISBN, from either form.
///
/// `None` when the text is not an ISBN whose check digit agrees, or when it
/// is a thirteen-digit one starting `979`, which was never issued in ten.
pub fn to_10(text: &str) -> Option<String> {
    let digits = digits(text);
    if !bibclean::isbn::valid(&digits) {
        return None;
    }
    match digits.len() {
        10 => Some(digits),
        13 if digits.starts_with("978") => {
            let body = &digits[3..12];
            let sum: u32 = body
                .bytes()
                .enumerate()
                .map(|(at, b)| u32::from(b - b'0') * (10 - at as u32))
                .sum();
            let check = match (11 - sum % 11) % 11 {
                10 => 'X',
                n => char::from(b'0' + n as u8),
            };
            Some(format!("{body}{check}"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_form_becomes_the_other() {
        assert_eq!(to_13("0-306-40615-2").as_deref(), Some("9780306406157"));
        assert_eq!(to_10("978-0-306-40615-7").as_deref(), Some("0306406152"));
        assert_eq!(to_13("9780306406157").as_deref(), Some("9780306406157"));
        assert_eq!(to_10("0306406152").as_deref(), Some("0306406152"));
    }

    #[test]
    fn a_check_digit_of_ten_is_written_x() {
        assert_eq!(to_10("9780804429573").as_deref(), Some("080442957X"));
        assert_eq!(to_13("080442957x").as_deref(), Some("9780804429573"));
    }

    #[test]
    fn a_979_isbn_has_no_ten_digit_form() {
        assert_eq!(to_10("979-10-90636-07-1"), None);
        assert_eq!(to_13("979-10-90636-07-1").as_deref(), Some("9791090636071"));
    }

    #[test]
    fn a_mistyped_isbn_is_not_converted_into_a_different_book() {
        assert_eq!(to_13("0-306-40615-3"), None);
        assert_eq!(to_10("978-0-306-40615-8"), None);
        assert_eq!(to_13("12345"), None);
    }
}
