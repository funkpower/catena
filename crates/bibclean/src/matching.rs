//! Deciding whether two records name the same work.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

/// Words in a title, as the comparison sees them.
static WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-z0-9]+").expect("valid"));

/// How much of the shorter title's distinctive vocabulary must appear in the
/// other before both are believed to name the same book.
const THRESHOLD: f64 = 0.8;

/// Words that carry no identifying weight.
///
/// Kept deliberately small: anything dropped here stops being able to tell
/// two books apart.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "by", "ed", "edition", "for", "from", "her", "his", "in", "its",
    "new", "of", "on", "or", "part", "revised", "second", "the", "their", "third", "to", "vol",
    "volume", "with",
];

/// The words of a title that identify it.
pub fn tokens(title: &str) -> BTreeSet<String> {
    WORDS
        .find_iter(&title.to_lowercase())
        .map(|found| found.as_str().to_string())
        .filter(|word| word.chars().count() > 2)
        .filter(|word| STOPWORDS.binary_search(&word.as_str()).is_err())
        .collect()
}

/// Whether two titles name the same work.
///
/// Character similarity is not usable here, and a real library shows exactly
/// why: "The Cambridge Companion to Aquinas" and "The Cambridge Companion to
/// Hume" are eighty-six per cent the same string, because they differ in one
/// short word out of five. Believing that attaches Hume's blurb and Hume's
/// subjects to Aquinas — silently, and to a whole shelf of series titles at
/// once.
///
/// So the comparison is of vocabulary rather than characters: nearly all of
/// the shorter title's distinctive words have to appear in the longer one.
/// A subtitle and a leading "The" are then free, while a swapped subject is
/// fatal.
pub fn same_work(ours: &str, theirs: &str) -> bool {
    let ours = tokens(ours);
    let theirs = tokens(theirs);
    if ours.is_empty() || theirs.is_empty() {
        return false;
    }
    let (shorter, longer) = if ours.len() <= theirs.len() {
        (&ours, &theirs)
    } else {
        (&theirs, &ours)
    };
    shorter.intersection(longer).count() as f64 / shorter.len() as f64 >= THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stopword_list_is_sorted_so_it_can_be_searched() {
        assert!(STOPWORDS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_subtitle_does_not_make_it_a_different_book() {
        assert!(same_work(
            "Arguing About War",
            "Arguing About War: A Study in Ethics"
        ));
        assert!(same_work("The Human Condition", "Human Condition"));
    }

    #[test]
    fn one_swapped_word_makes_it_a_different_book() {
        assert!(!same_work(
            "The Cambridge Companion to Aquinas",
            "The Cambridge Companion to Hume"
        ));
    }

    #[test]
    fn a_title_with_nothing_distinctive_in_it_matches_nothing() {
        assert!(!same_work("The Of And", "The Of And"));
        assert!(!same_work("", "The Human Condition"));
    }

    #[test]
    fn case_and_punctuation_do_not_matter() {
        assert!(same_work("THE HUMAN CONDITION!", "the human condition"));
    }
}
