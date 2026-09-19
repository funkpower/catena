//! Recognising a scripture heading, whatever an edition calls it.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use textkit::pattern::SPACE;

use crate::books::BOOKS;

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

/// Every spelling, lowercased, to the book it means and its place in the
/// canon.
static VARIANTS: LazyLock<HashMap<String, (&'static str, usize)>> = LazyLock::new(|| {
    BOOKS
        .iter()
        .enumerate()
        .flat_map(|(at, (canonical, variants))| {
            variants
                .iter()
                .map(move |variant| (variant.to_lowercase(), (*canonical, at)))
        })
        .collect()
});

/// Daggers and asterisks an edition marks a heading with.
static TRAILING_MARK: LazyLock<Regex> = LazyLock::new(|| pattern(&format!(r"[†‡*]+{SPACE}*$")));

static LEADING_THE: LazyLock<Regex> = LazyLock::new(|| pattern(&format!(r"(?i)^The{SPACE}+")));

/// A trailing chapter number, with the parenthetical alternates an edition
/// adds where two traditions number a psalm differently.
static CHAPTER: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"{SPACE}+\d+(?:{SPACE}*\([\d:,{SPACE}\-–—]+\))?{SPACE}*$"
    ))
});

/// "The First Book of the Kingdoms" and its like.
static VERBOSE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(?i)^(First|Second|Third|Fourth){SPACE}+Books?{SPACE}+of{SPACE}+(?:the{SPACE}+)?(.+?)$"
    ))
});

/// "The Book of Genesis".
static BOOK_OF: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(?i)^Books?{SPACE}+of{SPACE}+(?:the{SPACE}+)?(.+?)$"
    ))
});

/// Study material about a book, which belongs with it.
static ANNOTATIONS: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(?i)^(?:Annotations?|Notes?){SPACE}+for{SPACE}+(.+?)$"
    ))
});

/// The written-out ordinals, in the order the numbers go.
const ORDINALS: &[(&str, &str)] = &[
    ("first", "1"),
    ("fourth", "4"),
    ("second", "2"),
    ("third", "3"),
];

/// Books that a Septuagint edition prints as standalone headings but which
/// belong with Daniel.
const DANIEL_ADDITIONS: &[&str] = &[
    "bel and the dragon",
    "bel and the serpent",
    "prayer of azariah",
    "song of the three holy children",
    "song of the three young men",
    "susanna",
];

/// How many headings a document needs before its shape says anything.
const ENOUGH_HEADINGS: usize = 30;

/// The share of headings that must name a book of scripture.
const ENOUGH_SCRIPTURE: f64 = 0.4;

/// Strip the prefixes and the marks, so what is left can be looked up.
pub fn normalise(heading: &str) -> String {
    let text = heading.trim_matches(textkit::is_space);
    let text = TRAILING_MARK.replace(text, "");
    let text = text.trim_matches(textkit::is_space);
    let text = LEADING_THE.replace(text, "");
    let text = text.trim_matches(textkit::is_space).to_string();

    let text = match ANNOTATIONS.captures(&text) {
        Some(found) => found[1].trim_matches(textkit::is_space).to_string(),
        None => text,
    };

    let text = if let Some(found) = VERBOSE_NUMBER.captures(&text) {
        let ordinal = found[1].to_lowercase();
        let number = ORDINALS
            .binary_search_by_key(&ordinal.as_str(), |(word, _)| word)
            .map(|at| ORDINALS[at].1)
            .unwrap_or("");
        format!("{number} {}", found[2].trim_matches(textkit::is_space))
    } else if let Some(found) = BOOK_OF.captures(&text) {
        found[1].trim_matches(textkit::is_space).to_string()
    } else {
        text
    };

    CHAPTER
        .replace(&text, "")
        .trim_matches(textkit::is_space)
        .to_string()
}

/// Which book a heading names, and its place in the canon.
pub fn classify(heading: &str) -> Option<(&'static str, usize)> {
    let key = normalise(heading).to_lowercase();
    if let Some(hit) = VARIANTS.get(&key) {
        return Some(*hit);
    }
    DANIEL_ADDITIONS
        .binary_search(&key.as_str())
        .ok()
        .and_then(|_| VARIANTS.get("daniel").copied())
}

/// Whether a document is scripture rather than an anthology.
///
/// Scripture has a structurally different shape: every chapter is its own
/// top-level heading, so splitting it the ordinary way gives a thousand
/// fragments instead of the seventy-odd books anybody wants.
pub fn looks_like_scripture(headings: &[&str]) -> bool {
    if headings.len() < ENOUGH_HEADINGS {
        return false;
    }
    let named = headings
        .iter()
        .filter(|heading| classify(heading).is_some())
        .count();
    named as f64 / headings.len() as f64 > ENOUGH_SCRIPTURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_are_sorted_so_they_can_be_searched() {
        assert!(ORDINALS.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(DANIEL_ADDITIONS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_bare_name_is_recognised() {
        assert_eq!(classify("Genesis").map(|hit| hit.0), Some("Genesis"));
        assert_eq!(classify("Gen").map(|hit| hit.0), Some("Genesis"));
    }

    #[test]
    fn a_chapter_reference_names_its_book() {
        assert_eq!(classify("Genesis 14").map(|hit| hit.0), Some("Genesis"));
        assert_eq!(classify("Psalm 9 (9, 10)").map(|hit| hit.0), Some("Psalm"));
        assert_eq!(
            classify("Psalm 147 (147:12–20)").map(|hit| hit.0),
            Some("Psalm")
        );
    }

    #[test]
    fn the_verbose_forms_resolve_to_the_same_book() {
        assert_eq!(
            classify("The First Book of the Kingdoms").map(|hit| hit.0),
            Some("1 Kingdoms")
        );
        assert_eq!(
            classify("1 Samuel").map(|hit| hit.0),
            Some("1 Kingdoms"),
            "the Protestant spelling is the same book"
        );
        assert_eq!(
            classify("The Book of Genesis").map(|hit| hit.0),
            Some("Genesis")
        );
    }

    #[test]
    fn study_material_belongs_with_its_book() {
        assert_eq!(
            classify("Annotations for Job").map(|hit| hit.0),
            Some("Job")
        );
        assert_eq!(
            classify("Notes for Genesis").map(|hit| hit.0),
            Some("Genesis")
        );
    }

    #[test]
    fn a_trailing_mark_is_not_part_of_the_name() {
        assert_eq!(classify("Genesis †").map(|hit| hit.0), Some("Genesis"));
        assert_eq!(classify("Tobit*").map(|hit| hit.0), Some("Tobit"));
    }

    #[test]
    fn the_additions_to_daniel_belong_with_daniel() {
        for addition in ["Susanna", "Bel and the Dragon", "Prayer of Azariah"] {
            assert_eq!(
                classify(addition).map(|hit| hit.0),
                Some("Daniel"),
                "{addition}"
            );
        }
    }

    #[test]
    fn something_that_is_not_scripture_is_not_claimed() {
        assert_eq!(classify("Chapter One"), None);
        assert_eq!(classify("The Politics"), None);
        assert_eq!(classify(""), None);
    }

    #[test]
    fn the_canon_is_in_order() {
        let genesis = classify("Genesis").expect("a book").1;
        let revelation = classify("Revelation").expect("a book").1;
        assert!(genesis < revelation);
    }

    #[test]
    fn a_document_is_scripture_only_when_most_of_it_is() {
        let scripture: Vec<String> = (1..=50).map(|n| format!("Genesis {n}")).collect();
        let scripture: Vec<&str> = scripture.iter().map(String::as_str).collect();
        assert!(looks_like_scripture(&scripture));

        let anthology: Vec<&str> = vec!["The Politics"; 50];
        assert!(!looks_like_scripture(&anthology));

        // Too few headings to say anything either way.
        assert!(!looks_like_scripture(&scripture[..10]));
    }
}
