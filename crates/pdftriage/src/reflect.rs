//! Recovering a text layer that is not damaged, only permuted.

use crate::score::{ascii_text_score, word_shape};

/// Codes are stored reflected about this point.
const ABOUT: u32 = 0x120;
/// The band the reflection applies to.
const LOW: u32 = 0xa1;
const HIGH: u32 = 0xff;

/// Below this, the text as extracted does not read as ordinary text at all,
/// which is the precondition for trying the decode.
const LOOKS_DEAD: f64 = 0.35;
/// The decode has to improve the score by at least this much to be believed.
const MUST_IMPROVE: f64 = 0.30;
/// Landing in the ASCII range is not the same as landing in a language.
/// Measured on the two damaged books in the library this came from: tidy-case
/// 0.999 for the recovered text, 0.224 for a wrong-key decode that also
/// reflects into ASCII but into nonsense — `zgW$c_`, `iXYc$f\KO`.
const MIN_TIDY_CASE: f64 = 0.90;
const MIN_VOWELLED: f64 = 0.95;
/// Words needed before word shape means anything.
const MIN_WORDS: usize = 40;

/// Glyph slots outside the reflected band.
///
/// These are the ligatures, the quotes and dashes, and the accented letters.
/// Each was read off decoded context — `Univer<>ity` is an s, `Scienti<>c` an
/// fi, `Wolfenb<>ttel` a u-umlaut — rather than guessed. A slot with no
/// determined reading is left exactly as it stands: a rare glyph that still
/// looks odd is fixable later, and a wrong guess is not.
const SLOTS: &[(char, &str)] = &[
    ('-', "s"),
    ('A', "fl"),
    ('B', "fi"),
    ('H', "ij"),
    ('K', "\u{2019}"),
    ('L', "\u{201c}"),
    ('M', "\u{201d}"),
    ('N', "\u{2018}"),
    ('O', "\u{2014}"),
    ('P', "\u{2013}"),
    ('U', "\u{c0}"),
    ('a', "\u{f8}"),
    ('w', "\u{a9}"),
    ('\u{152}', "\u{ee}"),
    ('\u{160}', "\u{f1}"),
    ('\u{178}', "\u{c5}"),
    ('\u{17d}', "\u{ed}"),
    ('\u{2c6}', "\u{f2}"),
    ('\u{2dc}', "\u{e0}"),
    ('\u{2013}', "\u{e4}"),
    ('\u{2014}', "\u{e2}"),
    ('\u{2018}', "\u{e8}"),
    ('\u{2019}', "\u{e9}"),
    ('\u{201c}', "\u{e7}"),
    ('\u{201e}', "\u{fa}"),
    ('\u{2020}', "\u{f6}"),
    ('\u{2021}', "\u{f4}"),
    ('\u{2022}', "\u{fc}"),
    ('\u{2030}', "\u{f3}"),
    ('\u{2039}', "\u{ef}"),
    ('\u{203a}', "\u{d6}"),
    ('\u{2122}', "\u{e1}"),
];

/// Reverse a reflected-ASCII font layer, or `None` if that is not what this
/// is.
///
/// Some PDFs store every printable ASCII code reflected about a fixed point,
/// so that the text extracts as Latin-1 punctuation soup: `×²¬®±¼«½¬·±²` is
/// "Introduction". The mapping is exact, so the text comes back in full —
/// which is strictly better than an OCR pass that could only approximate it.
///
/// Deliberately self-validating rather than gated on which program produced
/// the file. The transform is applied only when the text as extracted does
/// not read as ordinary text at all *and* reflecting it makes it read as
/// ordinary text. A healthy book fails the first test, so it can never be
/// mangled by this; a Greek or Cyrillic book fails the second, because its
/// code points lie outside the band and the reflection changes nothing.
pub fn undo(text: &str) -> Option<String> {
    let before = ascii_text_score(text);
    if before >= LOOKS_DEAD {
        return None;
    }

    let mut decoded = String::with_capacity(text.len());
    for c in text.chars() {
        let code = c as u32;
        if (LOW..=HIGH).contains(&code) {
            decoded.push(char::from_u32(ABOUT - code).expect("the band reflects into ASCII"));
        } else {
            match slot(c) {
                Some(reading) => decoded.push_str(reading),
                None => decoded.push(c),
            }
        }
    }

    if ascii_text_score(&decoded) - before < MUST_IMPROVE {
        return None;
    }
    // Declining here leaves the file for a re-OCR, which is the right answer
    // when the key is not this one.
    let (vowelled, tidy_case) = word_shape(&decoded, MIN_WORDS)?;
    if tidy_case < MIN_TIDY_CASE || vowelled < MIN_VOWELLED {
        return None;
    }
    Some(decoded)
}

fn slot(c: char) -> Option<&'static str> {
    SLOTS
        .binary_search_by_key(&c, |(glyph, _)| *glyph)
        .ok()
        .map(|at| SLOTS[at].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode text the way such a PDF stores it, so a test can be written
    /// without shipping one of the books this was measured on.
    fn reflect(text: &str) -> String {
        text.chars()
            .map(|c| {
                let code = c as u32;
                let reflected = ABOUT - code;
                if (LOW..=HIGH).contains(&reflected) {
                    char::from_u32(reflected).expect("in range")
                } else {
                    c
                }
            })
            .collect()
    }

    fn prose() -> String {
        "The second estate held its land in exchange for service and the service was \
         military That much is agreed and what follows is not The court rolls do not \
         say how often the obligation was tested because the clerk who recorded it had \
         no vocabulary for the difference between a rebellion and a bad harvest"
            .to_string()
    }

    #[test]
    fn a_reflected_layer_comes_back_whole() {
        let stored = reflect(&prose());
        assert!(
            ascii_text_score(&stored) < 0.1,
            "the fixture is not reflected"
        );
        assert_eq!(undo(&stored).as_deref(), Some(prose().as_str()));
    }

    #[test]
    fn healthy_text_is_never_touched() {
        assert_eq!(undo(&prose()), None);
    }

    #[test]
    fn a_greek_layer_is_left_alone() {
        let greek = "Ἀθῆναι καὶ Μόσχα καὶ τὰ λοιπά ".repeat(20);
        assert_eq!(undo(&greek), None);
    }

    #[test]
    fn nonsense_that_reflects_into_ascii_is_refused() {
        // A layer scrambled with some other key reflects into ASCII too, but
        // into words with no vowels and no case pattern.
        let nonsense: String = (0..400)
            .map(|i| char::from_u32(0xa1 + (i * 7) % 0x5f).expect("in band"))
            .collect();
        assert_eq!(undo(&nonsense), None);
    }

    #[test]
    fn too_little_text_to_judge_is_refused() {
        assert_eq!(undo(&reflect("Introduction")), None);
    }

    #[test]
    fn the_slot_table_is_sorted_so_it_can_be_searched() {
        assert!(SLOTS.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
}
