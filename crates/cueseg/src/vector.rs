//! Turning a window of speech into something two windows can be compared by.
//!
//! Hashed bag of words, not embeddings. The question here is only "are these
//! two minutes about the same thing as the last two", and for that a count of
//! which content words appear is enough — and it costs no model, no GPU and
//! no network, which is what lets this run over an archive rather than over a
//! few files.

use std::sync::LazyLock;

use regex::Regex;

/// How many buckets a window's words are hashed into.
///
/// Small enough that two windows about the same subject collide on the same
/// buckets, large enough that two windows about different subjects mostly do
/// not.
pub const DIMENSIONS: usize = 256;

/// Everything that is not a letter, a digit, an apostrophe or whitespace.
static NOT_A_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^a-z0-9'\s\x{1c}-\x{1f}]+").expect("a valid pattern"));

/// Words too common in speech to say anything about the subject.
///
/// Longer than an ordinary stop list because this is transcribed speech: the
/// fillers, the discourse markers and the vague nouns that carry a
/// conversation carry no topic at all, and leaving them in makes every window
/// resemble every other.
const STOP: &[&str] = &[
    "about", "after", "ah", "all", "also", "and", "are", "back", "because", "been", "before",
    "but", "can", "come", "could", "did", "don", "even", "for", "from", "get", "going", "gonna",
    "got", "had", "has", "have", "here", "how", "into", "it's", "its", "just", "kind", "know",
    "let", "lets", "like", "little", "look", "lot", "mean", "much", "need", "not", "now", "oh",
    "ok", "okay", "one", "out", "over", "people", "really", "right", "said", "say", "says", "see",
    "should", "so", "sort", "still", "than", "that", "the", "their", "them", "then", "there",
    "they", "thing", "things", "think", "this", "uh", "um", "very", "want", "was", "well", "were",
    "what", "when", "who", "why", "will", "with", "would", "yeah", "yes", "you", "your",
];

/// The content words of a text.
pub fn tokens(text: &str) -> Vec<String> {
    let lowered = text.to_lowercase();
    let cleaned = NOT_A_WORD.replace_all(&lowered, " ");
    cleaned
        .split(textkit::is_space)
        .filter(|word| !word.is_empty())
        .filter(|word| word.chars().count() > 2 && !is_stop(word))
        .map(str::to_string)
        .collect()
}

fn is_stop(word: &str) -> bool {
    STOP.binary_search(&word).is_ok()
}

/// Hash a text's content words into a vector of counts.
pub fn lexical(text: &str, dimensions: usize) -> Vec<f64> {
    let mut vector = vec![0.0; dimensions];
    for token in tokens(text) {
        let at = fnv1a(&token) as usize % dimensions;
        vector[at] += 1.0;
    }
    vector
}

/// FNV-1a over the characters of a word.
///
/// Over the characters rather than the bytes, which is what the original
/// does. It makes no difference here — the tokeniser above leaves only ASCII
/// — but the two disagree for anything else, and a hash that disagrees puts
/// a word in a different bucket and moves a boundary.
fn fnv1a(word: &str) -> u32 {
    let mut hash: u32 = 2166136261;
    for c in word.chars() {
        hash ^= c as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

/// Cosine similarity, or zero when either side has no words at all.
pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for at in 0..n {
        dot += a[at] * b[at];
        norm_a += a[at] * a[at];
        norm_b += b[at] * b[at];
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// How much each window resembles the one after it.
pub fn consecutive_cosine(vectors: &[Vec<f64>]) -> Vec<f64> {
    vectors
        .windows(2)
        .map(|pair| cosine(&pair[0], &pair[1]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stop_list_is_sorted_so_it_can_be_searched() {
        assert!(STOP.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn tokens_are_content_words_only() {
        assert_eq!(
            tokens("So, you know, the MILL stood at the northern end!"),
            ["mill", "stood", "northern", "end"]
        );
    }

    #[test]
    fn short_words_and_punctuation_go() {
        assert_eq!(tokens("a bc def — ghi's"), ["def", "ghi's"]);
    }

    #[test]
    fn anything_that_is_not_a_latin_letter_becomes_a_break() {
        assert_eq!(tokens("café Москва mill"), ["caf", "mill"]);
    }

    #[test]
    fn the_hash_is_the_one_the_original_uses() {
        // FNV-1a over the characters, 32-bit.
        assert_eq!(fnv1a(""), 2166136261);
        assert_eq!(fnv1a("a"), 0xe40c292c);
        assert_eq!(fnv1a("mill"), fnv1a("mill"));
        assert_ne!(fnv1a("mill"), fnv1a("kiln"));
    }

    #[test]
    fn the_same_text_is_perfectly_similar_to_itself() {
        let vector = lexical("the mill stood at the northern end", DIMENSIONS);
        assert!((cosine(&vector, &vector) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn different_subjects_are_not_similar() {
        let a = lexical(
            "the mill stood at the northern end of the valley",
            DIMENSIONS,
        );
        let b = lexical(
            "parliament dissolved itself amid furious objections",
            DIMENSIONS,
        );
        assert!(cosine(&a, &b) < 0.2, "{}", cosine(&a, &b));
    }

    #[test]
    fn an_empty_window_resembles_nothing() {
        let empty = lexical("", DIMENSIONS);
        let full = lexical("the mill stood", DIMENSIONS);
        assert_eq!(cosine(&empty, &full), 0.0);
        assert_eq!(cosine(&empty, &empty), 0.0);
    }

    #[test]
    fn one_window_has_no_similarities_to_report() {
        assert!(consecutive_cosine(&[vec![1.0]]).is_empty());
        assert!(consecutive_cosine(&[]).is_empty());
    }
}
