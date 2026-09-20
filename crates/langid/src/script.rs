//! What writing system a text is in.
//!
//! For most of the world this is the whole answer: a text in Hangul is
//! Korean, a text in Hiragana is Japanese. For the Latin and Cyrillic
//! alphabets it narrows the field and no more, and the words have to decide.

use serde::{Deserialize, Serialize};

/// A writing system, as far as telling languages apart needs one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Script {
    /// The alphabet most of Europe writes in.
    Latin,
    /// Russian, Ukrainian, Bulgarian, Serbian and their neighbours.
    Cyrillic,
    /// Greek.
    Greek,
    /// Arabic, Persian, Urdu.
    Arabic,
    /// Hebrew and Yiddish.
    Hebrew,
    /// Chinese characters, which Japanese also uses.
    Han,
    /// Japanese syllabaries, which only Japanese uses.
    Kana,
    /// Korean.
    Hangul,
    /// Devanagari: Hindi, Marathi, Sanskrit.
    Devanagari,
    /// Something else, or too little to tell.
    Other,
}

/// How much of a text has to be in one script before it counts as that
/// script.
///
/// Well under half, because a book carries page numbers, names and quotations
/// in other alphabets, and an English word in a Russian paragraph does not
/// make it English.
const SHARE: f64 = 0.25;

/// The script a text is written in, and what share of its letters agree.
///
/// Counts letters only: digits and punctuation belong to every language and
/// would drown the signal in a text full of citations.
pub fn detect(text: &str) -> (Script, f64) {
    let mut counts = [0usize; 10];
    let mut letters = 0usize;

    for c in text.chars() {
        let Some(script) = of(c) else { continue };
        letters += 1;
        counts[index(script)] += 1;
    }
    if letters == 0 {
        return (Script::Other, 0.0);
    }

    // Kana before Han: a Japanese text is mostly Han characters with kana
    // between them, and it is the kana that say which language it is.
    if counts[index(Script::Kana)] as f64 / letters as f64 > 0.05 {
        return (
            Script::Kana,
            counts[index(Script::Kana)] as f64 / letters as f64,
        );
    }

    let (at, count) = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .expect("ten scripts");
    let share = *count as f64 / letters as f64;
    match share >= SHARE {
        true => (from_index(at), share),
        false => (Script::Other, share),
    }
}

fn index(script: Script) -> usize {
    match script {
        Script::Latin => 0,
        Script::Cyrillic => 1,
        Script::Greek => 2,
        Script::Arabic => 3,
        Script::Hebrew => 4,
        Script::Han => 5,
        Script::Kana => 6,
        Script::Hangul => 7,
        Script::Devanagari => 8,
        Script::Other => 9,
    }
}

fn from_index(at: usize) -> Script {
    [
        Script::Latin,
        Script::Cyrillic,
        Script::Greek,
        Script::Arabic,
        Script::Hebrew,
        Script::Han,
        Script::Kana,
        Script::Hangul,
        Script::Devanagari,
        Script::Other,
    ][at]
}

/// The script one character belongs to, or `None` if it is not a letter.
///
/// By code point range rather than by a Unicode property table, because the
/// ranges that matter here are few, stable, and cheaper than carrying the
/// table.
pub fn of(c: char) -> Option<Script> {
    let code = c as u32;
    Some(match code {
        0x41..=0x5A | 0x61..=0x7A => Script::Latin,
        0xC0..=0x24F => Script::Latin,    // Latin-1 through Extended-B
        0x1E00..=0x1EFF => Script::Latin, // Latin Extended Additional
        0x370..=0x3FF | 0x1F00..=0x1FFF => Script::Greek,
        0x400..=0x52F => Script::Cyrillic,
        0x590..=0x5FF => Script::Hebrew,
        0x600..=0x6FF | 0x750..=0x77F => Script::Arabic,
        0x900..=0x97F => Script::Devanagari,
        0x3040..=0x30FF => Script::Kana, // hiragana and katakana, adjacent
        0x4E00..=0x9FFF | 0x3400..=0x4DBF => Script::Han,
        0xAC00..=0xD7AF | 0x1100..=0x11FF => Script::Hangul,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_script_is_recognised_from_its_own_words() {
        for (text, script) in [
            ("The obligation ran both ways", Script::Latin),
            ("Молчание было единственным ответом", Script::Cyrillic),
            ("Καλημέρα κόσμε καὶ πάντες", Script::Greek),
            ("مرحبا بالعالم وأهلا", Script::Arabic),
            ("שלום עולם וברוך", Script::Hebrew),
            ("안녕하세요 세계 반갑습니다", Script::Hangul),
            ("नमस्ते दुनिया सब", Script::Devanagari),
        ] {
            assert_eq!(detect(text).0, script, "{text}");
        }
    }

    #[test]
    fn kana_settles_japanese_against_chinese() {
        // Both are mostly Han characters. The kana between them are the tell.
        assert_eq!(detect("これは日本語の文章です").0, Script::Kana);
        assert_eq!(detect("這是中文的句子沒有假名").0, Script::Han);
    }

    #[test]
    fn a_stray_foreign_word_does_not_change_the_script() {
        let mostly = "Молчание было единственным ответом на его вопрос about that";
        assert_eq!(detect(mostly).0, Script::Cyrillic);
    }

    #[test]
    fn digits_and_punctuation_are_not_evidence() {
        assert_eq!(detect("123 456 -- ... 789").0, Script::Other);
        assert_eq!(detect("").0, Script::Other);
    }

    #[test]
    fn accented_latin_is_still_latin() {
        assert_eq!(detect("naïve café résumé Grüße piñata").0, Script::Latin);
    }
}
