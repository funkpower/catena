//! What language a document is in.
//!
//! Two passes, because one does not do. The writing system settles most of
//! the world outright — a text in Hangul is Korean — and for the Latin and
//! Cyrillic alphabets it only narrows the field, so the commonest words
//! decide between what is left.
//!
//! No model, no download, no network. Function words make up a third of any
//! running text, and on a paragraph or more counting them is decisive. On a
//! single line it is not, which is why every answer carries a confidence and
//! why "could not tell" is one of the answers.

pub mod script;
pub mod words;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use script::Script;

/// How much text is worth looking at.
///
/// A book's language does not change after the first few pages, and reading
/// all of it to say so is waste.
pub const SAMPLE_CHARS: usize = 20_000;

/// What language a text turned out to be.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Verdict {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// What was read.
    pub path: String,
    /// ISO 639-1, the two-letter code. Absent when it could not be told.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// ISO 639-2/B, which is what the `doc` record carries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bibliographic: Option<String>,
    /// What the language is called in English.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The writing system, which is an answer even when the language is not.
    pub script: String,
    /// How far ahead the winner was, from zero to one. One means nothing else
    /// scored at all.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub confidence: f64,
    /// What was worth saying about the reading.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Verdict {
    fn version(&self) -> u32 {
        self.v
    }
}

/// Read a text's language.
pub fn detect(path: &str, text: &str) -> Verdict {
    let sample: String = text.chars().take(SAMPLE_CHARS).collect();
    let (script, share) = script::detect(&sample);

    let mut verdict = Verdict {
        v: corpus_core::V,
        path: path.to_string(),
        language: None,
        bibliographic: None,
        name: None,
        script: name_of(script).to_string(),
        confidence: 0.0,
        notes: Vec::new(),
    };

    // A script only one language uses is the whole answer, and no amount of
    // word counting would improve on it.
    if let Some(sole) = sole_language(script) {
        verdict.language = Some(sole.0.to_string());
        verdict.bibliographic = Some(sole.1.to_string());
        verdict.name = Some(sole.2.to_string());
        verdict.confidence = share;
        verdict
            .notes
            .push(format!("{} is written only in {}", sole.2, name_of(script)));
        return verdict;
    }

    let scored = words::score(&sample, script);
    let Some((best, best_score)) = scored.first() else {
        verdict.notes.push(match script {
            Script::Other => "no letters to go on".to_string(),
            script => format!(
                "nothing here knows the languages written in {}",
                name_of(script)
            ),
        });
        return verdict;
    };

    if *best_score == 0.0 {
        verdict
            .notes
            .push("none of the commonest words of any language it knows".to_string());
        return verdict;
    }

    // How far ahead of the runner-up, so a close call can be refused.
    let runner_up = scored.get(1).map_or(0.0, |(_, score)| *score);
    let ratio = match runner_up > 0.0 {
        true => best_score / runner_up,
        false => f64::INFINITY,
    };
    if ratio < words::MARGIN {
        verdict.notes.push(format!(
            "{} and {} score too alike to choose between",
            best.name, scored[1].0.name
        ));
        return verdict;
    }

    verdict.language = Some(best.code.to_string());
    verdict.bibliographic = Some(best.bibliographic.to_string());
    verdict.name = Some(best.name.to_string());
    verdict.confidence = match runner_up > 0.0 {
        true => (1.0 - runner_up / best_score).clamp(0.0, 1.0),
        false => 1.0,
    };
    verdict
}

/// The one language a script is used for, where there is only one.
fn sole_language(script: Script) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match script {
        Script::Kana => ("ja", "jpn", "Japanese"),
        Script::Han => ("zh", "chi", "Chinese"),
        Script::Hangul => ("ko", "kor", "Korean"),
        Script::Hebrew => ("he", "heb", "Hebrew"),
        Script::Arabic => ("ar", "ara", "Arabic"),
        Script::Devanagari => ("hi", "hin", "Hindi"),
        _ => return None,
    })
}

/// What a script is called.
pub fn name_of(script: Script) -> &'static str {
    match script {
        Script::Latin => "Latin",
        Script::Cyrillic => "Cyrillic",
        Script::Greek => "Greek",
        Script::Arabic => "Arabic",
        Script::Hebrew => "Hebrew",
        Script::Han => "Han",
        Script::Kana => "Kana",
        Script::Hangul => "Hangul",
        Script::Devanagari => "Devanagari",
        Script::Other => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_paragraph_of_english_is_english() {
        let found = detect(
            "x.md",
            "The obligation ran both ways, and contemporaries said so plainly whenever they were asked about it at all.",
        );
        assert_eq!(found.language.as_deref(), Some("en"));
        assert_eq!(found.bibliographic.as_deref(), Some("eng"));
        assert_eq!(found.script, "Latin");
        assert!(found.confidence > 0.0);
    }

    #[test]
    fn a_script_only_one_language_uses_needs_no_words() {
        for (text, code) in [
            ("안녕하세요 세계 반갑습니다 오늘은", "ko"),
            ("これは日本語の文章です", "ja"),
            ("מרחבי שלום עולם וברוך הבא", "he"),
        ] {
            assert_eq!(detect("x", text).language.as_deref(), Some(code), "{text}");
        }
    }

    #[test]
    fn nothing_to_read_is_no_answer_rather_than_a_wrong_one() {
        let found = detect("x", "12345 -- ...");
        assert_eq!(found.language, None);
        assert_eq!(found.script, "unknown");
        assert_eq!(found.notes, ["no letters to go on"]);
    }

    #[test]
    fn latin_words_no_language_claims_are_refused() {
        let found = detect("x", "zzzz qqqq xxxx vvvv wwww kkkk jjjj");
        assert_eq!(found.language, None);
        assert!(found.notes[0].starts_with("none of the commonest words"));
    }

    #[test]
    fn a_close_call_is_refused_rather_than_guessed() {
        // "de" is among the commonest words of four of these languages at
        // once, and on its own settles none of them.
        let found = detect("x", "de de de");
        assert_eq!(found.language, None, "{:?}", found.notes);
        assert!(found.notes[0].contains("too alike"), "{:?}", found.notes);
    }

    #[test]
    fn only_the_opening_is_read() {
        let long = format!(
            "{}{}",
            "The obligation ran both ways. ".repeat(2000),
            "xyzzy"
        );
        assert!(long.len() > SAMPLE_CHARS * 2);
        assert_eq!(detect("x", &long).language.as_deref(), Some("en"));
    }

    #[test]
    fn the_script_is_reported_even_when_the_language_is_not() {
        let found = detect("x", "zzzz qqqq xxxx vvvv");
        assert_eq!(found.script, "Latin");
        assert_eq!(found.language, None);
    }
}
