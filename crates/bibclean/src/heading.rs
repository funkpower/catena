//! An author field written as a library catalogue writes its headings, put
//! the way a title page would print the names.
//!
//! A catalogue files a person under the surname, adds dates of birth and
//! death to tell namesakes apart, and notes the person's role: "Lossky,
//! Vladimir, 1903-1958", "Vico, Giambattista (Author)". All of that is
//! right for a card and wrong for a byline, and it survives every export
//! that copies the heading into an author field.

use std::sync::LazyLock;

use regex::Regex;
use textkit::pattern::SPACE;

use crate::reasons::looks_like_person;

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

/// A role in brackets, or after a comma at the end: "(Author)", ", editor".
static ROLE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(?i){SPACE}*\({SPACE}*(?:author|editor|edt|eds?\.?|translator|trans\.?|compiler|illustrator){SPACE}*\)|,?{SPACE}+(?:author|editor|translator)\.?{SPACE}*$|{SPACE}*&{SPACE}*(?:editor|translator)s?{SPACE}*$"
    ))
});

/// Dates a catalogue adds to a heading: ", 1903-1958", ", 1973-",
/// ", b. 1950", ", d. 1622", ", fl. 1200".
static DATES: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(?i),?{SPACE}*(?:(?:b|d|fl|ca?)\.{SPACE}*)?\b(?:1\d{{3}}|20\d{{2}})\b(?:{SPACE}*[-–]{SPACE}*(?:(?:1\d{{3}}|20\d{{2}})\b)?)?\.?"
    ))
});

/// An initial run into the surname: "J.Dostal", "P.COPENHAVER".
static GLUED_INITIAL: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"\b(\p{Lu}\.)(\p{Lu}[\p{L}'-]{2,})"));

/// The fuller form of a name a catalogue adds after initials:
/// "Smith, Robert O. (Robert Owen)". Only after an initial, so a name given
/// in brackets for another reason, "Archimandrite Tikhon (Shevkunov)", stays.
static FULLER_FORM: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(\b\p{{Lu}}\.){SPACE}*\([\p{{Lu}}][\p{{L}}.' ]+\)"
    ))
});

/// What follows the comma in "Kenyon, Jr." or "Copleston, S.J." is part of
/// the name, not given names to put in front of it.
const SUFFIXES: &[&str] = &[
    "fils", "ii", "iii", "iv", "jr", "m.d", "md", "o.p", "o.s.b", "op", "osb", "pere", "phd",
    "père", "s.j", "sj", "sr",
];

/// Words a person's name does not contain. A field holding them is a title
/// or a sentence that a comma happens to split.
const NOT_IN_A_NAME: &[&str] = &[
    "a",
    "an",
    "and",
    "are",
    "be",
    "chapter",
    "collected",
    "do",
    "for",
    "how",
    "in",
    "is",
    "need",
    "of",
    "on",
    "the",
    "to",
    "volume",
    "we",
    "what",
    "why",
    "with",
    "works",
];

/// The names in a catalogue heading, printed as a title page prints them,
/// and what was wrong with the heading.
///
/// `None` when the field is already a byline, or when it cannot be read
/// with confidence: a field that is not names at all is not this function's
/// to rearrange.
///
/// A plain "Arendt, Hannah" comes back turned round, with only
/// `author_last_first` against it. Some catalogues keep authors in that
/// order on purpose, so [`crate::clean`] leaves such a field alone; a caller
/// whose author field is a byline can use this directly.
pub fn as_byline(author: &str) -> Option<(String, Vec<&'static str>)> {
    let original = author.trim_matches(textkit::is_space);
    if original.is_empty() || original.to_lowercase().contains("z-lib") {
        return None;
    }
    let mut faults = Vec::new();

    let mut text = GLUED_INITIAL.replace_all(original, "$1 $2").to_string();
    // A role between two names was what separated them: "Brian Leiter
    // (eds.) Michael Rosen (eds.)".
    let without_roles = ROLE.replace_all(&text, ";").to_string();
    let without_dates = DATES.replace_all(&without_roles, "").to_string();
    let without_fuller = FULLER_FORM.replace_all(&without_dates, "$1").to_string();
    if without_fuller.trim_end_matches([';', ' ']) != text.trim_end_matches([';', ' ']) {
        faults.push("author_role_suffix");
    }
    text = without_fuller;
    let text = text
        .trim_matches(|c: char| textkit::is_space(c) || c == ',' || c == ';')
        .to_string();

    let segments: Vec<String> = text
        .split([';', '&'])
        .map(|segment| split_segment(segment.trim_matches(textkit::is_space)))
        .collect::<Option<Vec<Vec<String>>>>()?
        .into_iter()
        .flatten()
        .collect();
    if segments.is_empty() {
        return None;
    }

    let mut names: Vec<String> = Vec::new();
    for segment in &segments {
        let (name, inverted) = turn_round(segment)?;
        if inverted && !faults.contains(&"author_last_first") {
            faults.push("author_last_first");
        }
        // The same person twice: "Ehrlich; Holdren & Ehrlich & Holdren".
        if !names.iter().any(|seen| seen.eq_ignore_ascii_case(&name)) {
            names.push(name);
        }
    }
    for name in &names {
        let words: Vec<&str> = name.split_whitespace().collect();
        if words.is_empty()
            || words.len() > 5
            || name.chars().any(|c| c.is_ascii_digit())
            || words
                .iter()
                .any(|word| NOT_IN_A_NAME.contains(&word.trim_matches('.').to_lowercase().as_str()))
            || !name.chars().next().is_some_and(char::is_uppercase)
            || !looks_like_person(name)
        {
            return None;
        }
    }
    for name in &names {
        crate::author::clean(name)?;
    }
    let byline = crate::author::normalise(&names.join(" & "))?;
    if byline == original {
        return None;
    }
    // A bracket left in the middle of a byline is a qualifier the rules did
    // not place: "Andrew (Physician) Baldwin".
    if byline.contains('(') {
        return None;
    }
    // A word still in capitals after normalising was not a name the rules
    // understood: "GARY'S MAC".
    if byline
        .split_whitespace()
        .any(|word| word.chars().filter(|c| c.is_uppercase()).count() >= 4 && !is_roman(word))
    {
        return None;
    }
    // Shouting, put in the case a title page prints names in: "STEVEN BOX".
    if faults.is_empty()
        && original.chars().any(char::is_alphabetic)
        && !original.chars().any(char::is_lowercase)
    {
        faults.push("allcaps_author");
    }
    // Only the separator changed: "Patrick Laude, Barry McDonald".
    if faults.is_empty() {
        faults.push("author_list_commas");
    }
    Some((byline, faults))
}

/// One segment between semicolons or ampersands, as the names in it.
///
/// "Laude, Patrick" is one name turned round; "Patrick Laude, Barry
/// McDonald" is two in order; "Kohut, Heinz, Ornstein, Paul H." is two
/// turned round and run together. `None` when it is none of these.
fn split_segment(segment: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = segment
        .split(',')
        .map(|part| part.trim_matches(textkit::is_space))
        .filter(|part| !part.is_empty())
        .collect();
    // "John F., M.D. Greden": a suffix opening a part means the commas are
    // not separating names at all.
    if parts
        .iter()
        .skip(1)
        .any(|part| part.split_whitespace().next().is_some_and(is_suffix) && !is_suffix(part))
    {
        return None;
    }
    match parts.len() {
        0 => Some(Vec::new()),
        1 => Some(vec![parts[0].to_string()]),
        2 if is_suffix(parts[1]) => Some(vec![format!("{}, {}", parts[0], parts[1])]),
        2 if is_suffix(parts[0]) => None,
        2 if words(parts[0]) >= 2 && words(parts[1]) >= 2 && !ends_in_initial(parts[1]) => {
            // Two names in order, unless the second is someone's given
            // names: "Martin Luther King, Martin" does not happen, but
            // "van Gogh, Vincent Willem" does, so a lowercase particle
            // opening the first part keeps it a turned-round name.
            if parts[0].starts_with(char::is_lowercase) {
                Some(vec![format!("{}, {}", parts[0], parts[1])])
            } else {
                Some(parts.iter().map(|part| (*part).to_string()).collect())
            }
        }
        2 => Some(vec![format!("{}, {}", parts[0], parts[1])]),
        n if parts
            .iter()
            .all(|part| words(part) >= 2 && !ends_in_initial(part)) =>
        {
            let _ = n;
            Some(parts.iter().map(|part| (*part).to_string()).collect())
        }
        n if n % 2 == 0 && parts.iter().step_by(2).all(|surname| words(surname) <= 2) => Some(
            parts
                .chunks(2)
                .map(|pair| format!("{}, {}", pair[0], pair[1]))
                .collect(),
        ),
        _ => None,
    }
}

/// A name with its given names put first, and whether it had to be turned.
fn turn_round(name: &str) -> Option<(String, bool)> {
    let Some((surname, given)) = name.split_once(',') else {
        return Some((name.trim_matches(textkit::is_space).to_string(), false));
    };
    let surname = surname.trim_matches(textkit::is_space);
    let given = given.trim_matches(|c: char| textkit::is_space(c) || c == ',');
    if is_suffix(given) {
        return Some((format!("{surname}, {given}"), false));
    }
    // "Heinz." from "Kohut, Heinz.": the full stop closed the heading, not
    // an initial, and has no place in the middle of a byline.
    let given = match given.split_whitespace().last() {
        Some(last)
            if last.ends_with('.')
                && last.trim_end_matches('.').chars().count() > 1
                && !last.trim_end_matches('.').contains('.') =>
        {
            given.trim_end_matches('.')
        }
        _ => given,
    };
    if surname.is_empty()
        || given.is_empty()
        || words(surname) > 3
        || words(given) > 4
        || surname.eq_ignore_ascii_case(given.trim_end_matches('.'))
        // "Ian Fleming, Ian": the given name already in front.
        || surname
            .split_whitespace()
            .next()
            .is_some_and(|first| first.eq_ignore_ascii_case(given.trim_end_matches('.')))
    {
        return None;
    }
    Some((format!("{given} {surname}"), true))
}

fn is_roman(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| "IVXLC".contains(c))
}

fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn ends_in_initial(text: &str) -> bool {
    text.split_whitespace()
        .last()
        .is_some_and(|last| last.trim_end_matches('.').chars().count() == 1)
}

pub(crate) fn is_suffix(text: &str) -> bool {
    // "Md/phd": degrees run together.
    text.split('/').all(|part| {
        SUFFIXES.contains(
            &part
                .trim_matches(|c: char| textkit::is_space(c) || c == '.')
                .to_lowercase()
                .as_str(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn byline(author: &str) -> Option<String> {
        as_byline(author).map(|(name, _)| name)
    }

    #[test]
    fn a_name_in_capitals_is_named_as_shouting() {
        assert_eq!(
            as_byline("STEVEN BOX"),
            Some(("Steven Box".to_string(), vec!["allcaps_author"]))
        );
        assert_eq!(
            as_byline("Patrick Laude, Barry McDonald").map(|(_, faults)| faults),
            Some(vec!["author_list_commas"])
        );
    }

    #[test]
    fn a_heading_is_turned_round() {
        for (heading, want) in [
            ("Tolkien, J. R. R.", "J. R. R. Tolkien"),
            ("Borella, Jean", "Jean Borella"),
            ("Aurelius, Marcus", "Marcus Aurelius"),
            ("Raya, Joseph M", "Joseph M. Raya"),
            ("Lossky, Vladimir, 1903-1958", "Vladimir Lossky"),
            ("Hamilton, Kevin, 1973- author", "Kevin Hamilton"),
            ("Vico, Giambattista(Author)", "Giambattista Vico"),
            ("Smith, Robert O. (Robert Owen), 1974-", "Robert O. Smith"),
            ("Swinburne, Richard.", "Richard Swinburne"),
            (
                "Kohut, Heinz.,Ornstein, Paul H.",
                "Heinz Kohut & Paul H. Ornstein",
            ),
            (
                "Goodin, Robert E. & Pettit, Philip",
                "Robert E. Goodin & Philip Pettit",
            ),
            ("Mills, Robert;", "Robert Mills"),
            ("Martin, George RR", "George RR Martin"),
            ("Hauser, Henri, 1866-1946", "Henri Hauser"),
            (
                "Sheen, Fulton J. (Fulton John), 1895-1979",
                "Fulton J. Sheen",
            ),
        ] {
            assert_eq!(byline(heading).as_deref(), Some(want), "{heading}");
        }
    }

    #[test]
    fn a_list_in_order_is_joined() {
        for (heading, want) in [
            (
                "Patrick Laude, Barry McDonald",
                "Patrick Laude & Barry McDonald",
            ),
            (
                "Eleonore Stump (Editor), Norman Kretzmann (Editor)",
                "Eleonore Stump & Norman Kretzmann",
            ),
            (
                "Brian Leiter (eds.) Michael Rosen (eds.)",
                "Brian Leiter & Michael Rosen",
            ),
            ("KENNETH HAYNES (edt)", "Kenneth Haynes"),
        ] {
            assert_eq!(byline(heading).as_deref(), Some(want), "{heading}");
        }
    }

    #[test]
    fn what_is_already_a_byline_or_not_a_name_is_left_alone() {
        for author in [
            "Hannah Arendt",
            "Ralph E. Kenyon, Jr.",
            "Frederick Copleston, S.J.",
            "Chapter 1, Do We Need Philosophy",
            "The Club of Rome, 2008",
            "Volume 4 in the Collected Works of Georges Florovsky. (Florovsky, Georges) (z-library.sk, 1lib.sk, z-lib.sk)",
            "Jr., Robert P. Granacher",
            "Scientific American, Inc.",
            "Alexandre Dumas, Pere",
            "Onasander , Onasander",
            "John F., M.D. Greden",
            "Archimandrite Tikhon (Shevkunov)",
            "JC Beall",
            "Experience & Judgment 001-396",
            "GARY'S MAC",
            "Pope Leo XIII",
            "Baldwin, Andrew (Physician), author",
            "Gibbs Smith, Publisher",
            "Ian Fleming, Ian",
            "",
        ] {
            assert_eq!(byline(author), None, "{author}");
        }
    }
}
