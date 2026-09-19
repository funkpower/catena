//! A volume marker in a title, and the series it implies.

use std::collections::HashMap;
use std::sync::LazyLock;

use fancy_regex::Regex;

/// The roman numerals a volume is ever written in.
///
/// Twenty is where real collected editions stop; past that the numeral is far
/// more often a century or a chapter than a volume.
static ROMAN: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {
    [
        ("i", 1),
        ("ii", 2),
        ("iii", 3),
        ("iv", 4),
        ("v", 5),
        ("vi", 6),
        ("vii", 7),
        ("viii", 8),
        ("ix", 9),
        ("x", 10),
        ("xi", 11),
        ("xii", 12),
        ("xiii", 13),
        ("xiv", 14),
        ("xv", 15),
        ("xvi", 16),
        ("xvii", 17),
        ("xviii", 18),
        ("xix", 19),
        ("xx", 20),
    ]
    .into_iter()
    .collect()
});

/// "Vol 3", "Volume IX", "Book 1", "Part 2", "Tome II".
///
/// The unit word is required. A bare trailing number is far more often a
/// year, an edition, or noise than a volume.
static VOLUME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?ix) [\s,:;.\-–—(\[]* \b(?:vol(?:ume)?s?|bk|book|part|pt|tome)\b [\s.:\#\-]* (?P<num>\d{1,3}|[ivxlc]{1,7}) \b[\s)\]]*")
        .expect("a valid pattern")
});

/// The largest volume number worth believing.
///
/// Past two hundred it is a page, a year or a catalogue number that happens
/// to sit after the word "book".
const MAX_VOLUME: u32 = 200;

/// Parse `9` or `IX` to nine. `None` for anything that is neither.
///
/// Digits in any script, because the pattern that finds them matches the
/// Unicode class and a catalogue in Arabic writes its volumes in Arabic
/// figures.
pub fn roman_or_int(text: &str) -> Option<u32> {
    let text = text.trim().to_lowercase();
    if text.is_empty() {
        return None;
    }
    if text.chars().all(char::is_numeric) {
        let number = textkit::decimal(&text);
        return match (1..=MAX_VOLUME).contains(&number) {
            true => Some(number),
            false => None,
        };
    }
    ROMAN.get(text.as_str()).copied()
}

/// Split a volume marker off a title, giving the series and the number.
///
/// `None` when there is no marker, or when removing it leaves nothing that
/// could be a series name.
pub fn parse_volume(title: &str) -> Option<(String, u32)> {
    let title = title.trim();
    if title.is_empty() {
        return None;
    }

    let found = VOLUME.captures(title).ok().flatten()?;
    let whole = found.get(0)?;
    let volume = roman_or_int(found.name("num")?.as_str())?;

    // What comes before the marker is the series. Whatever follows is this
    // volume's own subtitle — "The Art of Computer Programming, Volume 3:
    // Sorting and Searching" is volume 3 *of* the former, and gluing the
    // halves back together gives a title nobody wrote.
    let mut name = &title[..whole.start()];

    // The trailing-parenthetical convention puts the series inside the
    // brackets: "The Song of the Sirin (Raven Son Book 1)" is book one of
    // "Raven Son", not of "The Song of the Sirin (Raven Son".
    if let Some(open) = name.rfind(['(', '[']) {
        let after = &name[open..];
        if !after.contains(')') && !after.contains(']') {
            name = &name[open + 1..];
        }
    }

    let name = collapse_spaces(name);
    let name = name.trim_matches(|c: char| " ,:;.-–—([".contains(c));
    let name = name.trim_end_matches(|c: char| " ,:;.-–—".contains(c));

    // Nothing before the marker means the title never named a series. "Vol.
    // 2. PLATO, EPICTETUS, MARCUS AURELIUS" is a volume of something whose
    // name is simply not in the field, and filing it under its own contents
    // is worse than leaving it unfiled.
    if name.chars().count() < 4 || !has_word(name) {
        return None;
    }
    Some((name.to_string(), volume))
}

/// Runs of whitespace down to one space.
fn collapse_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut spaces = 0usize;
    for c in text.chars() {
        match c.is_whitespace() {
            true => spaces += 1,
            false => {
                if spaces > 0 {
                    out.push(' ');
                    spaces = 0;
                }
                out.push(c);
            }
        }
    }
    if spaces > 0 {
        out.push(' ');
    }
    out
}

/// Whether there are three letters in a row anywhere, which is the least a
/// series name can be and still be a name.
fn has_word(text: &str) -> bool {
    let mut run = 0;
    for c in text.chars() {
        run = match c.is_ascii_alphabetic() {
            true => run + 1,
            false => 0,
        };
        if run >= 3 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_and_roman_both_read_as_numbers() {
        assert_eq!(roman_or_int("9"), Some(9));
        assert_eq!(roman_or_int("IX"), Some(9));
        assert_eq!(roman_or_int(" ix "), Some(9));
        assert_eq!(roman_or_int("xx"), Some(20));
    }

    #[test]
    fn what_is_not_a_volume_number_is_not_one() {
        assert_eq!(roman_or_int(""), None);
        assert_eq!(roman_or_int("0"), None);
        assert_eq!(roman_or_int("201"), None);
        assert_eq!(roman_or_int("xxi"), None, "past twenty is not a volume");
        assert_eq!(roman_or_int("nine"), None);
    }

    #[test]
    fn a_volume_written_in_another_script_is_still_a_volume() {
        assert_eq!(roman_or_int("٣"), Some(3));
        assert_eq!(roman_or_int("١٢"), Some(12));
        // A fraction is numeric and is not a digit run.
        assert_eq!(roman_or_int("½"), None);
    }

    #[test]
    fn the_series_is_what_comes_before_the_marker() {
        assert_eq!(
            parse_volume("The Philokalia Vol 3"),
            Some(("The Philokalia".to_string(), 3))
        );
        assert_eq!(
            parse_volume("Collected Works of C.G. Jung, Volume 9"),
            Some(("Collected Works of C.G. Jung".to_string(), 9))
        );
    }

    #[test]
    fn a_volumes_own_subtitle_is_not_part_of_the_series_name() {
        assert_eq!(
            parse_volume("The Art of Computer Programming, Volume 3: Sorting and Searching"),
            Some(("The Art of Computer Programming".to_string(), 3))
        );
    }

    #[test]
    fn a_series_named_inside_a_parenthetical_is_the_series() {
        assert_eq!(
            parse_volume("The Song of the Sirin (Raven Son Book 1)"),
            Some(("Raven Son".to_string(), 1))
        );
    }

    #[test]
    fn a_title_that_never_named_a_series_is_left_unfiled() {
        // A Harvard Classics volume, whose series name is not in the field.
        assert_eq!(
            parse_volume("Vol. 2. PLATO, EPICTETUS, MARCUS AURELIUS"),
            None
        );
        assert_eq!(parse_volume("Book 1"), None);
    }

    #[test]
    fn a_bare_number_is_not_a_volume() {
        // Far more often a year, an edition, or noise.
        assert_eq!(parse_volume("Being and Time 1927"), None);
        assert_eq!(parse_volume("Ethics 2"), None);
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert_eq!(parse_volume(""), None);
        assert_eq!(parse_volume("   "), None);
    }

    #[test]
    fn roman_volumes_read_the_same_as_arabic_ones() {
        assert_eq!(
            parse_volume("A History of Philosophy, Vol. IV"),
            Some(("A History of Philosophy".to_string(), 4))
        );
    }
}
