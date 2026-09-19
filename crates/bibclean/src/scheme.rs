//! A closed subject scheme, and what it says about a heading.
//!
//! A catalogue that lets anything be a subject ends up with forty thousand of
//! them and no way to browse. A scheme is the other approach: a fixed list of
//! headings, everything else mapped onto one or dropped.
//!
//! The scheme itself is supplied rather than compiled in. Which subjects a
//! collection uses, and what its parents are, is the most collection-specific
//! thing there is — a theology library and a science library agree on nothing
//! here — so it arrives as a file and this only knows what to do with one.

use std::collections::HashMap;
use std::sync::LazyLock;

use fancy_regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Titles that say the book is *about* somebody rather than by them, or is a
/// study rather than a story.
///
/// General reference rather than collection-specific: a Companion is a
/// Companion on any shelf.
static STUDY_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(companion to|guidebook to|very short introduction|letters of|collected letters|memoirs?\b|autobiograph|\bdiary\b|\bessays\b|\blectures\b|\bspeeches\b|a life of|biography of)\b")
        .expect("a valid pattern")
});

/// A subject scheme: what the headings are, and how they relate.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scheme {
    /// Every heading the scheme allows, in the order they should be listed.
    #[serde(default)]
    pub subjects: Vec<String>,
    /// Spellings that mean one of the subjects: alias to subject.
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    /// Headings too broad or too incidental to keep, whatever they say.
    #[serde(default)]
    pub drop: Vec<String>,
    /// Which subject each subject sits under, child to parent.
    #[serde(default)]
    pub parents: HashMap<String, String>,
    /// Subjects that mean the work is a study, so it cannot also be fiction.
    #[serde(default)]
    pub blocks_fiction: Vec<String>,
    /// The genres a study blocks.
    #[serde(default)]
    pub fiction_genres: Vec<String>,
}

impl Scheme {
    /// Whether the scheme says anything at all.
    pub fn is_empty(&self) -> bool {
        self.subjects.is_empty()
    }

    /// The subject a heading means, or `None` if it is not one.
    ///
    /// Exact first, then an alias. A heading on the drop list is not a
    /// subject however it is spelled, and the brackets a catalogue wraps its
    /// own annotations in are stripped before the comparison.
    pub fn canonical(&self, name: &str) -> Option<&str> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let lower = name.to_lowercase();
        let bare = lower.trim_matches(|c: char| "*[]".contains(c));

        if self.drop.iter().any(|drop| {
            let drop = drop.to_lowercase();
            let drop = drop.trim_matches(|c: char| "*[]".contains(c));
            drop == lower || drop == bare
        }) {
            return None;
        }
        if let Some(found) = self.subjects.iter().find(|s| s.to_lowercase() == lower) {
            return Some(found);
        }
        self.aliases
            .iter()
            .find(|(alias, _)| alias.to_lowercase() == lower)
            .map(|(_, subject)| subject.as_str())
    }

    /// Every chosen subject, with the parent of each carried alongside it.
    ///
    /// Phenomenology becomes Philosophy and Phenomenology, so a shelf browsed
    /// by the parent still finds the child. Returned in scheme order, which
    /// is the order a reader expects to see them listed.
    pub fn with_parents(&self, subjects: &[String]) -> Vec<String> {
        let mut chosen: Vec<&str> = Vec::new();
        for subject in subjects {
            if !self.subjects.iter().any(|s| s == subject) {
                continue;
            }
            chosen.push(subject);
            if let Some(parent) = self.parents.get(subject) {
                chosen.push(parent);
            }
        }
        self.in_order(&chosen)
    }

    /// Drop the fiction genres where the work is plainly a study.
    ///
    /// A historical novel may be fiction and history at once. A life of
    /// somebody, a Companion, or a writer's diary may not be a novel.
    pub fn without_impossible_fiction(
        &self,
        subjects: &[String],
        title: Option<&str>,
    ) -> Vec<String> {
        let study = title.is_some_and(|title| STUDY_TITLE.is_match(title).unwrap_or(false));
        let blocked = subjects
            .iter()
            .any(|s| self.blocks_fiction.iter().any(|block| block == s));

        let kept: Vec<&str> = subjects
            .iter()
            .map(String::as_str)
            .filter(|subject| {
                !(study || blocked) || !self.fiction_genres.iter().any(|genre| genre == subject)
            })
            .collect();
        self.in_order(&kept)
    }

    /// The subjects a book should carry: the chosen ones, their parents, and
    /// nothing the title rules out.
    pub fn compose(&self, subjects: &[String], title: Option<&str>) -> Vec<String> {
        self.without_impossible_fiction(&self.with_parents(subjects), title)
    }

    /// Whether a book has no real subject yet.
    ///
    /// Untagged, or tagged only with headings the scheme does not recognise —
    /// which is what a catalogue built from filenames looks like.
    pub fn needs_subject(&self, existing: &[String]) -> bool {
        !existing.iter().any(|tag| self.canonical(tag).is_some())
    }

    /// The given headings, deduplicated and in scheme order.
    fn in_order(&self, chosen: &[&str]) -> Vec<String> {
        self.subjects
            .iter()
            .filter(|subject| chosen.contains(&subject.as_str()))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheme() -> Scheme {
        Scheme {
            subjects: [
                "Philosophy",
                "Phenomenology",
                "History",
                "Biography",
                "Fiction",
                "Science Fiction",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            aliases: [("Philosophie", "Philosophy"), ("SF", "Science Fiction")]
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            drop: ["General", "Humanities", "Non-Fiction"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            parents: [("Phenomenology", "Philosophy")]
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            blocks_fiction: vec!["Biography".into()],
            fiction_genres: vec!["Fiction".into(), "Science Fiction".into()],
        }
    }

    fn subjects(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_heading_in_the_scheme_is_itself() {
        assert_eq!(scheme().canonical("Philosophy"), Some("Philosophy"));
        assert_eq!(scheme().canonical("philosophy"), Some("Philosophy"));
    }

    #[test]
    fn an_alias_is_the_subject_it_means() {
        assert_eq!(scheme().canonical("Philosophie"), Some("Philosophy"));
        assert_eq!(scheme().canonical("sf"), Some("Science Fiction"));
    }

    #[test]
    fn a_heading_on_the_drop_list_is_not_a_subject() {
        assert_eq!(scheme().canonical("General"), None);
        // Including inside the brackets a catalogue wraps its own notes in.
        assert_eq!(scheme().canonical("[General]"), None);
        assert_eq!(scheme().canonical("*Humanities*"), None);
    }

    #[test]
    fn a_heading_the_scheme_never_heard_of_is_not_a_subject() {
        assert_eq!(scheme().canonical("Underwater Basketry"), None);
        assert_eq!(scheme().canonical(""), None);
        assert_eq!(scheme().canonical("   "), None);
    }

    #[test]
    fn a_child_carries_its_parent() {
        assert_eq!(
            scheme().with_parents(&subjects(&["Phenomenology"])),
            ["Philosophy", "Phenomenology"]
        );
    }

    #[test]
    fn subjects_come_back_in_scheme_order_however_they_went_in() {
        assert_eq!(
            scheme().with_parents(&subjects(&["History", "Philosophy"])),
            ["Philosophy", "History"]
        );
    }

    #[test]
    fn a_heading_outside_the_scheme_is_dropped_rather_than_carried() {
        assert_eq!(
            scheme().with_parents(&subjects(&["Basketry"])),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_biography_is_not_also_a_novel() {
        assert_eq!(
            scheme().without_impossible_fiction(&subjects(&["Biography", "Fiction"]), None),
            ["Biography"]
        );
    }

    #[test]
    fn a_title_that_says_study_rules_out_fiction_on_its_own() {
        for title in [
            "The Cambridge Companion to Nietzsche",
            "Logic: A Very Short Introduction",
            "A Life of Wrangel",
            "The Diary of a Writer",
        ] {
            assert_eq!(
                scheme()
                    .without_impossible_fiction(&subjects(&["Fiction", "History"]), Some(title)),
                ["History"],
                "{title}"
            );
        }
    }

    #[test]
    fn a_historical_novel_may_still_be_both() {
        assert_eq!(
            scheme().without_impossible_fiction(
                &subjects(&["Fiction", "History"]),
                Some("War and Peace")
            ),
            ["History", "Fiction"]
        );
    }

    #[test]
    fn composing_does_the_parents_and_the_fiction_rule_at_once() {
        assert_eq!(
            scheme().compose(
                &subjects(&["Phenomenology", "Fiction"]),
                Some("A Life of Husserl")
            ),
            ["Philosophy", "Phenomenology"]
        );
    }

    #[test]
    fn a_book_tagged_only_with_rubbish_still_needs_a_subject() {
        assert!(scheme().needs_subject(&[]));
        assert!(scheme().needs_subject(&subjects(&["General", "ebook"])));
        assert!(!scheme().needs_subject(&subjects(&["General", "Philosophy"])));
    }

    #[test]
    fn an_empty_scheme_says_nothing_about_anything() {
        let empty = Scheme::default();
        assert!(empty.is_empty());
        assert_eq!(empty.canonical("Philosophy"), None);
        assert!(empty.needs_subject(&subjects(&["Philosophy"])));
    }
}
