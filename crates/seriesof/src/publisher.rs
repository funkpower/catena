//! Publisher-collection shorthand: "Aquinas (Companion)" against "Cambridge".
//!
//! The publishers and their collections are general reference data — a
//! Cambridge Companion is the same thing on every shelf in the world — of the
//! same kind as the publisher names elsewhere in this suite.

use std::collections::HashMap;
use std::sync::LazyLock;

use fancy_regex::Regex;

/// A trailing parenthetical short enough to be a collection name.
static PAREN_KIND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(([^)]{3,40})\)\s*$").expect("a valid pattern"));

/// A trailing year, parenthesised or not, that a filename picked up.
static TRAILING_YEAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\s*[\[(]\s*(?:19|20)\d{2}\s*[\])]\s*|\s*[.,\-]\s*(?:19|20)\d{2}\s*)$")
        .expect("a valid pattern")
});

/// A second trailing parenthetical, once the first has been taken as the kind.
static TRAILING_PAREN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[\[(][^\])]{0,40}[\])]\s*$").expect("a valid pattern"));

/// The publishers this shorthand appears with, and the name their collections
/// are published under.
static PUBLISHERS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    [
        ("cambridge", "Cambridge"),
        ("oxford", "Oxford"),
        ("blackwell", "Blackwell"),
        ("wiley blackwell", "Blackwell"),
        ("routledge", "Routledge"),
        ("suny", "SUNY"),
        ("palgrave", "Palgrave"),
        ("springer", "Springer"),
    ]
    .into_iter()
    .collect()
});

/// What each kind of collection is called, and what its volumes are titled.
///
/// The title is `None` where the real one is not derivable from the shorthand
/// — an "Anthology" or a "Studies" volume is not titled by any one pattern,
/// and guessing produces things nobody published. For those only the series
/// is proposed, which is the half that fixes browsing.
static KINDS: LazyLock<HashMap<&'static str, (Option<&'static str>, &'static str)>> =
    LazyLock::new(|| {
        [
            ("companion", (None, "{pub} Companions")),
            ("companions", (None, "{pub} Companions")),
            ("concise", (None, "{pub} Concise Companions")),
            ("handbook", (None, "{pub} Handbooks")),
            (
                "guidebook",
                (
                    Some("{pub} Philosophy Guidebook to {topic}"),
                    "{pub} Philosophy Guidebooks",
                ),
            ),
            (
                "very short introduction",
                (
                    Some("{topic}: A Very Short Introduction"),
                    "Very Short Introductions",
                ),
            ),
            (
                "contemporary debates",
                (
                    Some("Contemporary Debates in {topic}"),
                    "Contemporary Debates",
                ),
            ),
            (
                "contemporary introductions",
                (None, "{pub} Contemporary Introductions"),
            ),
            ("anthology", (None, "{pub} Anthologies")),
            ("studies", (None, "{pub} Studies")),
        ]
        .into_iter()
        .collect()
    });

/// Titles that depend on who published them.
///
/// The houses do not name their volumes alike, and getting it wrong renames a
/// book. Cambridge and Routledge front theirs with the imprint; Blackwell does
/// not — its Companions to Philosophy are "A Companion to Nietzsche", while a
/// few of its religion titles genuinely are "The Blackwell Companion to …".
/// That is unresolvable from the shorthand, so Blackwell gets its series and
/// no title claim.
static PUBLISHER_TITLES: LazyLock<HashMap<(&'static str, &'static str), &'static str>> =
    LazyLock::new(|| {
        [
            (
                ("Cambridge", "companion"),
                "The Cambridge Companion to {topic}",
            ),
            (
                ("Cambridge", "companions"),
                "The Cambridge Companion to {topic}",
            ),
            (
                ("Routledge", "companion"),
                "The Routledge Companion to {topic}",
            ),
            (
                ("Routledge", "companions"),
                "The Routledge Companion to {topic}",
            ),
            (("Oxford", "handbook"), "The Oxford Handbook of {topic}"),
        ]
        .into_iter()
        .collect()
    });

/// Reconstruct a title and a series from publisher shorthand.
///
/// Gives `(title, series)`, where the title is `None` for the kinds whose
/// real titles do not follow one pattern. `None` altogether when the title
/// carries no recognised parenthetical, or the author is not a publisher this
/// shorthand shows up with.
pub fn publisher_series(title: &str, author: &str) -> Option<(Option<String>, String)> {
    let title = title.trim();
    // Lowercased and trimmed, but runs of space inside are left alone: the
    // table is matched against exactly, and a doubled space in an author
    // field is as likely to be two different names run together as a typo.
    let author = author.trim().to_lowercase();
    if title.is_empty() {
        return None;
    }
    let publisher = *PUBLISHERS.get(author.as_str())?;

    let found = PAREN_KIND.captures(title).ok().flatten()?;
    let kind = collapse(&found.get(1)?.as_str().trim().to_lowercase());
    let (title_template, series_template) = *KINDS.get(kind.as_str())?;

    let trim = |text: &str| {
        text.trim_matches(|c: char| " ,:;.-–—".contains(c))
            .to_string()
    };
    let topic = trim(&title[..found.get(0)?.start()]);
    let topic = trim(&TRAILING_YEAR.replace(&topic, ""));
    let topic = trim(&TRAILING_PAREN.replace(&topic, ""));
    if topic.chars().count() < 2 {
        return None;
    }

    // The per-publisher title first; the per-kind one only covers collections
    // that name themselves the same way whoever publishes them.
    let template = PUBLISHER_TITLES
        .get(&(publisher, kind.as_str()))
        .copied()
        .or(title_template);
    Some((
        template.map(|template| fill(template, publisher, &topic)),
        fill(series_template, publisher, &topic),
    ))
}

/// A template with its two placeholders filled.
fn fill(template: &str, publisher: &str, topic: &str) -> String {
    template
        .replace("{pub}", publisher)
        .replace("{topic}", topic)
}

/// Runs of whitespace down to one space.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cambridge_companion_is_reconstructed_whole() {
        assert_eq!(
            publisher_series("Aquinas (Companion)", "Cambridge"),
            Some((
                Some("The Cambridge Companion to Aquinas".to_string()),
                "Cambridge Companions".to_string()
            ))
        );
    }

    #[test]
    fn blackwell_gets_its_series_and_no_title_claim() {
        // Its companions are "A Companion to Nietzsche", not "The Blackwell
        // Companion to Nietzsche", and the shorthand cannot tell which.
        assert_eq!(
            publisher_series("Nietzsche (Companion)", "Blackwell"),
            Some((None, "Blackwell Companions".to_string()))
        );
    }

    #[test]
    fn a_collection_that_names_itself_the_same_way_everywhere_needs_no_publisher() {
        assert_eq!(
            publisher_series("Logic (Very Short Introduction)", "Oxford"),
            Some((
                Some("Logic: A Very Short Introduction".to_string()),
                "Very Short Introductions".to_string()
            ))
        );
    }

    #[test]
    fn a_trailing_year_is_not_part_of_the_topic() {
        for title in [
            "Christian Ethics (2004) (Companion)",
            "Christian Ethics.2009 (Companion)",
            "Christian Ethics [2006] (Companion)",
        ] {
            let (proposed, _) = publisher_series(title, "Cambridge").expect("a series");
            assert_eq!(
                proposed.as_deref(),
                Some("The Cambridge Companion to Christian Ethics"),
                "{title}"
            );
        }
    }

    #[test]
    fn a_second_parenthetical_is_not_part_of_the_topic_either() {
        let (proposed, _) =
            publisher_series("British Drama [Contemporary] (Companion)", "Cambridge")
                .expect("a series");
        assert_eq!(
            proposed.as_deref(),
            Some("The Cambridge Companion to British Drama")
        );
    }

    #[test]
    fn an_author_who_is_not_a_publisher_proposes_nothing() {
        assert_eq!(publisher_series("Aquinas (Companion)", "Fergus Kerr"), None);
    }

    #[test]
    fn a_parenthetical_that_is_not_a_collection_proposes_nothing() {
        assert_eq!(
            publisher_series("Aquinas (Second Edition)", "Cambridge"),
            None
        );
        assert_eq!(publisher_series("Aquinas", "Cambridge"), None);
    }

    #[test]
    fn a_shorthand_with_no_topic_left_proposes_nothing() {
        assert_eq!(publisher_series("(Companion)", "Cambridge"), None);
    }

    #[test]
    fn the_publisher_is_read_however_it_is_cased_or_padded() {
        assert!(publisher_series("Ethics (Companion)", "  WILEY BLACKWELL ").is_some());
        // But a doubled space inside is a different string, not a typo to fix.
        assert!(publisher_series("Ethics (Companion)", "wiley  blackwell").is_none());
    }
}
