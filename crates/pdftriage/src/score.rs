//! Cheap, script-blind measurements of whether text is text.

use std::sync::LazyLock;

use regex::Regex;
use unicode_general_category::{GeneralCategory, get_general_category};

/// Characters that ordinary Latin-script text is made of.
///
/// Prose in any Latin-script language sits near 1.0 on the share of its
/// non-space characters that are in here; a text layer stored through a
/// reflected font sits near 0.0. The em dash is in the set despite not being
/// ASCII, because it is ordinary in a book and its absence would penalise
/// well-typeset text.
const ASCII_TEXT: &str = concat!(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
    ".,;:!?'\"()[]{}-\u{2014}/&%$#@*+=<>|~^_`\\"
);

/// Words of three letters or more, which is enough to have a shape.
static WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z]{3,}").expect("valid"));

static VOWEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[aeiouyAEIOUY]").expect("valid"));

/// Whether a character is a letter, by Python's definition.
///
/// Python's `str.isalpha` is true for the letter categories and nothing else.
/// Rust's [`char::is_alphabetic`] is true for the Unicode `Alphabetic`
/// property, which also covers some seventeen hundred combining marks —
/// including `U+0363`–`U+036F`, the medieval superscript letters that appear
/// in exactly the scanned manuscripts this has to judge. The thresholds below
/// were measured against the narrower definition.
pub fn is_letter(c: char) -> bool {
    matches!(
        get_general_category(c),
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
    )
}

/// The non-space characters of a text, which every share below is taken over.
pub fn body(text: &str) -> Vec<char> {
    text.chars().filter(|c| !textkit::is_space(*c)).collect()
}

/// Share of non-space characters that are ordinary Latin-script text.
pub fn ascii_text_score(text: &str) -> f64 {
    let body = body(text);
    if body.is_empty() {
        return 0.0;
    }
    let ordinary = body.iter().filter(|c| ASCII_TEXT.contains(**c)).count();
    ordinary as f64 / body.len() as f64
}

/// Share of the non-space characters that are letters, in any alphabet.
pub fn letter_share(body: &[char]) -> f64 {
    if body.is_empty() {
        return 0.0;
    }
    body.iter().filter(|c| is_letter(**c)).count() as f64 / body.len() as f64
}

/// Share of the non-space characters sitting in the Latin-1 band, which is
/// where a broken character map falls back to.
pub fn latin1_share(body: &[char]) -> f64 {
    if body.is_empty() {
        return 0.0;
    }
    let banded = body
        .iter()
        .filter(|c| (0xa1..=0xff).contains(&(**c as u32)))
        .count();
    banded as f64 / body.len() as f64
}

/// The share of words carrying a vowel, and the share in a sane case
/// pattern. `None` when there are too few words to judge.
///
/// A cheap, script-blind proxy for "is this a language", used to tell a real
/// decode from one done with the wrong key.
pub fn word_shape(text: &str, min_words: usize) -> Option<(f64, f64)> {
    let words: Vec<&str> = WORDS.find_iter(text).map(|found| found.as_str()).collect();
    if words.len() < min_words {
        return None;
    }
    let vowelled = words.iter().filter(|word| VOWEL.is_match(word)).count();
    let tidy = words.iter().filter(|word| tidy_case(word)).count();
    Some((
        vowelled as f64 / words.len() as f64,
        tidy as f64 / words.len() as f64,
    ))
}

/// Whether a word is all lower case, Capitalised, or ALL CAPS — which real
/// words are, and `zgW$c_` is not.
fn tidy_case(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let rest = chars.as_str();
    let all_lower = word.chars().all(|c| c.is_ascii_lowercase());
    let all_upper = word.chars().all(|c| c.is_ascii_uppercase());
    let title = first.is_ascii_uppercase() && rest.chars().all(|c| c.is_ascii_lowercase());
    all_lower || all_upper || title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_scores_near_one() {
        let score = ascii_text_score("The second estate held its land in exchange for service.");
        assert!(score > 0.95, "{score}");
    }

    #[test]
    fn a_latin1_soup_scores_near_zero() {
        let score = ascii_text_score("\u{d7}\u{b2}\u{ac}\u{ae}\u{b1}\u{bc}\u{ab}\u{bd}");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn empty_text_scores_zero_rather_than_dividing_by_nothing() {
        assert_eq!(ascii_text_score(""), 0.0);
        assert_eq!(ascii_text_score("   \n  "), 0.0);
        assert_eq!(letter_share(&[]), 0.0);
        assert_eq!(latin1_share(&[]), 0.0);
    }

    #[test]
    fn a_medieval_combining_letter_is_not_a_letter_here() {
        // Python's isalpha says no; Rust's is_alphabetic says yes.
        assert!(!is_letter('\u{363}'));
        assert!('\u{363}'.is_alphabetic());
        assert!(is_letter('a'));
        assert!(is_letter('\u{3b1}'));
        assert!(!is_letter('1'));
    }

    #[test]
    fn word_shape_needs_enough_words_to_judge() {
        assert_eq!(word_shape("one two three", 40), None);
    }

    #[test]
    fn real_words_are_vowelled_and_tidily_cased() {
        let text = "The second estate held its land in exchange for service and the service \
                    was military That much is agreed what follows is not The rolls do not say \
                    how often the obligation was tested and the clerk had no vocabulary for it "
            .repeat(3);
        let (vowelled, tidy) = word_shape(&text, 40).expect("enough words");
        assert!(vowelled > 0.95, "{vowelled}");
        assert!(tidy > 0.95, "{tidy}");
    }

    #[test]
    fn the_case_test_is_what_it_says() {
        assert!(tidy_case("lower"));
        assert!(tidy_case("Title"));
        assert!(tidy_case("UPPER"));
        assert!(!tidy_case("mIxEd"));
        assert!(!tidy_case("aBc"));
    }
}
