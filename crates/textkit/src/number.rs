//! Reading numbers out of text the way Python reads them.

/// The value of a run of decimal digits, Unicode included.
///
/// Digits are matched by the Unicode class in the patterns these tools port,
/// and Python's `int()` accepts every decimal digit, not just the ASCII ones
/// — so `page٤٢` is page forty-two there and has to be here too. Non-digits
/// are skipped, and a number too large for the field saturates rather than
/// wrapping.
pub fn decimal(text: &str) -> u32 {
    let mut value: u32 = 0;
    for digit in text.chars().filter_map(digit_value) {
        value = value.saturating_mul(10).saturating_add(digit);
    }
    value
}

/// What one decimal digit is worth, in any script.
///
/// [`char::to_digit`] knows only the ASCII digits. Every Unicode decimal
/// block, though, runs zero to nine in consecutive codepoints, so counting
/// back to the first non-digit gives the value without a lookup table.
fn digit_value(c: char) -> Option<u32> {
    if let Some(digit) = c.to_digit(10) {
        return Some(digit);
    }
    if !c.is_numeric() {
        return None;
    }
    let code = c as u32;
    let mut value = 9;
    for back in 1..=9 {
        let previous = code.checked_sub(back).and_then(char::from_u32);
        match previous {
            Some(previous) if previous.is_numeric() => continue,
            _ => {
                value = back - 1;
                break;
            }
        }
    }
    // Confirm it really is a run of ten. Fractions and the other numeric
    // characters sit in short runs, and would otherwise be read as digits:
    // `½` follows `¼`, which would make it a one.
    let zero = code - value;
    let ten = (zero..zero + 10).all(|code| char::from_u32(code).is_some_and(char::is_numeric));
    ten.then_some(value)
}

/// Round half to even, which is what Python's `round` does and what Rust's
/// does not: `round(2.5)` is 2 there and 3 here.
///
/// It matters wherever a rounded number decides something — how many
/// chapters a recording gets, what a percentage in a report reads as — and
/// the two rules disagree exactly on the values that land on a boundary,
/// which is where a disagreement is most likely to be noticed.
pub fn round_half_to_even(value: f64) -> f64 {
    let rounded = value.round();
    if (value - value.trunc()).abs() == 0.5 && rounded % 2.0 != 0.0 {
        rounded - value.signum()
    } else {
        rounded
    }
}

/// A fraction as a whole-number percentage, rounded the same way Python
/// rounds it.
pub fn percent(fraction: f64) -> String {
    format!("{}%", round_half_to_even(fraction * 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_digits_read_as_themselves() {
        assert_eq!(decimal("<!-- Page 412 -->"), 412);
    }

    #[test]
    fn other_decimal_digits_read_too() {
        assert_eq!(decimal("٤٢"), 42);
        assert_eq!(decimal("４２"), 42);
    }

    #[test]
    fn every_digit_of_a_script_is_worth_what_it_looks_like() {
        for (zero, name) in [('\u{660}', "arabic-indic"), ('\u{ff10}', "fullwidth")] {
            for offset in 0..10u32 {
                let digit = char::from_u32(zero as u32 + offset).expect("a digit");
                assert_eq!(digit_value(digit), Some(offset), "{name} {digit}");
            }
        }
    }

    #[test]
    fn a_character_that_is_not_a_digit_is_worth_nothing() {
        for c in ['x', ' ', '-', '½'] {
            assert_eq!(digit_value(c), None, "{c}");
        }
    }

    #[test]
    fn nothing_reads_as_zero() {
        assert_eq!(decimal("page"), 0);
    }

    #[test]
    fn rounding_goes_to_even_on_a_half() {
        assert_eq!(round_half_to_even(0.5), 0.0);
        assert_eq!(round_half_to_even(1.5), 2.0);
        assert_eq!(round_half_to_even(2.5), 2.0);
        assert_eq!(round_half_to_even(-2.5), -2.0);
        assert_eq!(round_half_to_even(2.4), 2.0);
    }

    #[test]
    fn a_percentage_reads_as_a_whole_number() {
        assert_eq!(percent(0.0), "0%");
        // Not 33: a third of a percent is a hair above the halfway point
        // once it is a binary float, so it rounds up rather than to even.
        assert_eq!(percent(0.335), "34%");
        assert_eq!(percent(1.0), "100%");
        assert_eq!(percent(0.125), "12%");
    }

    #[test]
    fn an_absurd_number_saturates_rather_than_wrapping() {
        assert_eq!(decimal(&"9".repeat(40)), u32::MAX);
    }
}
