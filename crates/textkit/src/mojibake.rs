//! UTF-8 text that was read as Windows-1252 somewhere along the way.
//!
//! "américain" stored as UTF-8 and decoded one byte at a time comes out as
//! "amÃ©ricain". The repair is the reverse trip: turn each character back into
//! the Windows-1252 byte it was decoded from and read those bytes as UTF-8.
//! It is certain rather than guessed, because ordinary text almost never
//! survives that trip: a real "café" becomes the lone byte `0xe9`, which is not
//! UTF-8, and is left alone.

/// The text as it was before it was misread, or `None` if it was not misread.
///
/// Text misread twice is unwound twice. Anything that cannot make the trip
/// cleanly, every character back to a byte and every byte into valid UTF-8,
/// is returned as `None` rather than half-repaired.
pub fn repair(text: &str) -> Option<String> {
    let mut current = once(text)?;
    // Twice-misread text is common enough; three times is not, and the bound
    // keeps a pathological input from looping.
    for _ in 0..2 {
        match once(&current) {
            Some(again) => current = again,
            None => break,
        }
    }
    Some(current)
}

/// Whether the text reads as UTF-8 misread as Windows-1252.
pub fn is_mojibake(text: &str) -> bool {
    once(text).is_some()
}

fn once(text: &str) -> Option<String> {
    if text.is_ascii() {
        return None;
    }
    let bytes = text.chars().map(byte_of).collect::<Option<Vec<u8>>>()?;
    let repaired = String::from_utf8(bytes).ok()?;
    (repaired != text).then_some(repaired)
}

/// The Windows-1252 byte a character decodes from, if there is one.
///
/// The five positions Windows-1252 leaves undefined are taken as themselves,
/// the way browsers and most decoders leave them.
fn byte_of(c: char) -> Option<u8> {
    let code = u32::from(c);
    if code < 0x80 || (0xa0..=0xff).contains(&code) {
        return u8::try_from(code).ok();
    }
    HIGH_CONTROLS
        .iter()
        .position(|&high| high == c)
        .and_then(|at| u8::try_from(0x80 + at).ok())
}

/// What Windows-1252 decodes the bytes 0x80 to 0x9f as.
const HIGH_CONTROLS: [char; 32] = [
    '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}',
    '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accented_latin_comes_back() {
        assert_eq!(
            repair("L'impÃ©rialisme amÃ©ricain").as_deref(),
            Some("L'impérialisme américain")
        );
        assert_eq!(
            repair("Onvriers du temps passÃ© [XVe-XVIe siÃ¨cles]").as_deref(),
            Some("Onvriers du temps passé [XVe-XVIe siècles]")
        );
    }

    #[test]
    fn punctuation_from_the_high_controls_comes_back() {
        // U+2019 is E2 80 99, and 0x80 and 0x99 are among the bytes
        // Windows-1252 decodes to something other than themselves.
        assert_eq!(repair("Donâ€™t Look").as_deref(), Some("Don\u{2019}t Look"));
        assert_eq!(repair("Ãœber Kant").as_deref(), Some("Über Kant"));
    }

    #[test]
    fn misread_twice_is_unwound_twice() {
        assert_eq!(repair("cafÃƒÂ©").as_deref(), Some("café"));
    }

    #[test]
    fn real_text_is_left_alone() {
        for text in [
            "café",
            "Été",
            "Ægypt",
            "Ångström",
            "naïve résumé",
            "Œuvres complètes",
            "plain ascii",
            "Москва",
            "東京",
            "",
        ] {
            assert_eq!(repair(text), None, "{text}");
            assert!(!is_mojibake(text), "{text}");
        }
    }

    #[test]
    fn half_repairable_text_is_left_alone() {
        // The Cyrillic cannot be a Windows-1252 character, so the misread
        // half cannot be undone without guessing about the rest.
        assert_eq!(repair("amÃ©ricain Москва"), None);
    }
}
