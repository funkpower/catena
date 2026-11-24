//! A series and a number written in front of the title, as a filename
//! writes them: "Known Space-038-The Soft Weapon", "Ice and Fire 2 - A Clash
//! of Kings", "James Bond - 032 - Blast from the Past".

use std::sync::LazyLock;

use fancy_regex::Regex;

use crate::volume::roman_or_int;

/// The series, the number between separators, then the title. The number
/// has to stand between separators on both sides, so "The 200-Year Plot"
/// and "The 12-Month Gardener" are titles with numbers in them, not
/// numbered titles.
static NUMBERED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<series>[\p{L}][^:\d]{1,60}?)(?:\s+(?P<a>\d{1,3})\s+[-–—.]\s+|\s*[-–—]\s*(?P<b>\d{1,3})\s*[-–—]\s*)(?P<title>\p{Lu}.*)$",
    )
    .expect("a valid pattern")
});

/// A series name, a volume and the title without them, or `None` when the
/// title does not open that way.
pub fn parse_numbered(title: &str) -> Option<(String, u32, String)> {
    let found = NUMBERED.captures(title.trim()).ok().flatten()?;
    let series = found["series"].trim_matches(|c: char| c.is_whitespace() || "-–—".contains(c));
    if series.split_whitespace().count() > 6 || !series.chars().any(char::is_alphabetic) {
        return None;
    }
    // "00 - The Hedge Knight": a prequel numbered zero is still in the
    // series, and zero is its place.
    let number = found.name("a").or_else(|| found.name("b"))?.as_str();
    let volume = if number.chars().all(|c| c == '0') {
        0
    } else {
        roman_or_int(number)?
    };
    let rest = found["title"].trim().trim_end_matches('.').trim();
    if rest.is_empty() {
        return None;
    }
    Some((series.to_string(), volume, rest.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_numbered_title_gives_series_volume_and_title() {
        for (title, series, volume, rest) in [
            (
                "Known Space-038-The Soft Weapon",
                "Known Space",
                38,
                "The Soft Weapon",
            ),
            (
                "Ice and Fire 2 - A Clash of Kings",
                "Ice and Fire",
                2,
                "A Clash of Kings",
            ),
            (
                "Ice and Fire 00 - The Hedge Knight",
                "Ice and Fire",
                0,
                "The Hedge Knight",
            ),
            (
                "James Bond - 032 - Blast from the Past",
                "James Bond",
                32,
                "Blast from the Past",
            ),
            ("Time Odyssey 2 - Sunstorm", "Time Odyssey", 2, "Sunstorm"),
            (
                "Known Space-040-Destroyer of Worlds.",
                "Known Space",
                40,
                "Destroyer of Worlds",
            ),
        ] {
            assert_eq!(
                parse_numbered(title),
                Some((series.to_string(), volume, rest.to_string())),
                "{title}"
            );
        }
    }

    #[test]
    fn a_title_with_a_number_in_it_is_not_numbered() {
        for title in [
            "Infiltration: The 200-Year Plot to Destroy the Catholic Church",
            "The 12-Month Vegetable Gardener's Handbook",
            "The Cambridge History of Medieval Political Thought C.350-C.1450",
            "The Chronicles of Narnia Complete 7-Book Collection",
            "CUUS309-FM p1-16",
            "01 - The Three Musketeers",
            "Being and Time",
        ] {
            assert_eq!(parse_numbered(title), None, "{title}");
        }
    }
}
