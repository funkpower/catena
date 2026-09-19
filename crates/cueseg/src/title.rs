//! A title taken from what was said, for a chapter nobody has named yet.

use std::sync::LazyLock;

use regex::Regex;

/// Hesitations. A chapter called "Um Yeah So I Think" is worse than no title.
static FILLER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(um+|uh+|ah+|er+)\b").expect("a valid pattern"));

/// The most common discourse marker in recorded speech.
static YOU_KNOW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\byou know\b").expect("a valid pattern"));

static SPACE_RUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\s\x{1c}-\x{1f}]+").expect("a valid pattern"));

/// Anything that does not belong in a title.
static NOT_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^a-zA-Z0-9'-]").expect("a valid pattern"));

/// How many words a title gets.
const WORDS: usize = 8;

/// Take a title from the first sentence of a chapter.
///
/// Extractive, and no cleverer than it looks: the first thing said in a
/// chapter is usually what the chapter is about, and where it is not, a
/// reader can see that at a glance and a model can replace it later.
pub fn extract(excerpt: &str) -> String {
    let cleaned = FILLER.replace_all(excerpt, " ");
    let cleaned = YOU_KNOW.replace_all(&cleaned, " ");
    let cleaned = SPACE_RUN.replace_all(&cleaned, " ");
    let cleaned = cleaned.trim_matches(textkit::is_space);

    let sentence = textkit::sentences(cleaned).first().copied().unwrap_or("");
    let title: Vec<String> = sentence
        .split(textkit::is_space)
        .filter(|word| !word.is_empty())
        .take(WORDS)
        .map(|word| NOT_TITLE.replace_all(word, "").into_owned())
        .filter(|word| !word.is_empty())
        .map(|word| capitalise(&word))
        .collect();

    if title.is_empty() {
        "Topic".to_string()
    } else {
        title.join(" ")
    }
}

/// First letter up, the rest down — so that a transcript's shouting and its
/// mid-sentence capitals come out looking like a title either way.
fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_sentence_becomes_the_title() {
        assert_eq!(
            extract("The mill stood at the northern end. Its wheel was undershot."),
            "The Mill Stood At The Northern End"
        );
    }

    #[test]
    fn a_title_is_at_most_eight_words() {
        let title = extract("one two three four five six seven eight nine ten.");
        assert_eq!(title.split(' ').count(), 8);
    }

    #[test]
    fn fillers_and_discourse_markers_go() {
        assert_eq!(
            extract("Um, so, you know, uh the mill stood there."),
            "So The Mill Stood There"
        );
    }

    #[test]
    fn punctuation_inside_a_word_is_kept_and_around_it_is_not() {
        assert_eq!(
            extract("(the) mill's well-built wheel."),
            "The Mill's Well-built Wheel"
        );
    }

    #[test]
    fn shouting_comes_out_as_a_title() {
        assert_eq!(extract("THE MILL STOOD."), "The Mill Stood");
    }

    #[test]
    fn nothing_usable_still_gives_a_title() {
        assert_eq!(extract(""), "Topic");
        assert_eq!(extract("... !!! ???"), "Topic");
        assert_eq!(extract("um uh ah"), "Topic");
    }
}
