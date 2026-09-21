//! Filing a book's printed subject headings onto a shelf someone actually has.
//!
//! A cataloguing block states subjects in Library of Congress form, which is
//! precise, deep, and no use as a shelf. Real blocks from real books read:
//!
//! ```text
//! Political science – Early works to 1800
//! English literature—Middle English, 1100–1500—History and criticism
//! Methodology. 2.Anthropic principle
//! Hobbes, Thomas, 1588–1679. Leviathan
//! ```
//!
//! Nobody browses by those. A library is browsed by a short closed list its
//! owner chose, and the job here is to get from the first to the second.
//!
//! The mapping is deliberately dumb: a heading is matched against the terms of
//! the shelf, longest first, and the first hit wins. It infers nothing, invents
//! nothing, and returns nothing at all rather than guess — a heading that
//! matches no term is reported unfiled, with its raw text kept, because a
//! subject silently dropped is indistinguishable from a book with no subjects.

use std::collections::BTreeSet;

/// A closed list of headings, with the words that file a book under each.
///
/// Parsed from a plain text file: one heading per line, optionally followed by
/// `=` and the comma-separated terms that should match it. A heading with no
/// terms matches its own name.
///
/// ```text
/// Byzantine History = byzantium, byzantine, constantinople, eastern roman
/// Patristics = church fathers, patristic, patrology
/// Ethics
/// ```
#[derive(Debug, Clone, Default)]
pub struct Shelf {
    /// (term, heading), longest term first so the most specific wins.
    terms: Vec<(String, String)>,
    headings: BTreeSet<String>,
}

impl Shelf {
    /// Read a shelf from the list format above. Blank lines and `#` comments
    /// are skipped.
    pub fn parse(text: &str) -> Self {
        let mut terms: Vec<(String, String)> = Vec::new();
        let mut headings = BTreeSet::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (heading, rest) = match line.split_once('=') {
                Some((h, r)) => (h.trim(), r.trim()),
                None => (line, ""),
            };
            if heading.is_empty() {
                continue;
            }
            headings.insert(heading.to_string());
            terms.push((fold(heading), heading.to_string()));
            for term in rest.split(',') {
                let term = fold(term);
                if !term.is_empty() {
                    terms.push((term, heading.to_string()));
                }
            }
        }
        // Longest first: "ancient philosophy" must beat "philosophy", or every
        // period of thought files under the parent and the shelf does no work.
        terms.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        terms.dedup_by(|a, b| a.0 == b.0);
        Self { terms, headings }
    }

    /// Whether the shelf lists no headings, in which case it files nothing.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Whether a name is one of the shelf's own headings.
    pub fn has(&self, heading: &str) -> bool {
        self.headings.contains(heading)
    }

    /// The heading a single raw subject files under, if any.
    pub fn file_one(&self, raw: &str) -> Option<&str> {
        let hay = fold(raw);
        if hay.is_empty() {
            return None;
        }
        self.terms
            .iter()
            .find(|(term, _)| contains_word(&hay, term))
            .map(|(_, heading)| heading.as_str())
    }

    /// File a book's raw headings, in the shelf's own order, without repeats.
    ///
    /// Returns the headings that matched and the raw subjects that did not, so
    /// a caller can tell "this book is not about anything on the shelf" from
    /// "this book printed no subjects".
    pub fn file(&self, raw: &[String]) -> Filed {
        let mut on = Vec::new();
        let mut unfiled = Vec::new();
        for subject in raw {
            match self.file_one(subject) {
                Some(h) => {
                    if !on.iter().any(|x: &String| x == h) {
                        on.push(h.to_string());
                    }
                }
                None => unfiled.push(subject.clone()),
            }
        }
        Filed { on, unfiled }
    }
}

/// What a shelf made of a book's printed subjects.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filed {
    /// Headings from the shelf, in the order the book's subjects hit them.
    pub on: Vec<String>,
    /// Subjects that matched no term, kept verbatim.
    pub unfiled: Vec<String>,
}

/// Lowercase, and reduce every run of non-alphanumerics to one space.
///
/// Cataloguing blocks separate with em dashes, en dashes, full stops and plain
/// hyphens, sometimes three of them in one heading, and OCR turns any of those
/// into any other. None of that should decide whether a book gets shelved.
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = true; // no leading space
    for c in s.chars() {
        if c.is_alphanumeric() {
            for l in c.to_lowercase() {
                out.push(l);
            }
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Whether `needle` appears in `hay` on whole-word boundaries.
///
/// Substring matching would file "Islam" under any book mentioning "Islamic"
/// — which is right — but would also file "Art" under "Cartography", which is
/// not. Both strings are already folded, so the boundary is a space.
fn contains_word(hay: &str, needle: &str) -> bool {
    if needle.is_empty() || needle.len() > hay.len() {
        return false;
    }
    hay.match_indices(needle).any(|(i, _)| {
        let before = i == 0 || hay.as_bytes()[i - 1] == b' ';
        let end = i + needle.len();
        let after = end == hay.len() || hay.as_bytes()[end] == b' ';
        before && after
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shelf() -> Shelf {
        Shelf::parse(
            "# a shelf\n\
             Philosophy = philosophy, philosophical\n\
             Ancient Philosophy = ancient philosophy, plato, aristotle\n\
             Political Science = political science, politics\n\
             Theology = theology, christianity\n\
             Byzantine History = byzantium, byzantine, constantinople\n\
             Ethics\n",
        )
    }

    #[test]
    fn a_heading_with_no_terms_matches_its_own_name() {
        assert_eq!(shelf().file_one("Ethics — early works"), Some("Ethics"));
    }

    #[test]
    fn the_most_specific_term_wins() {
        // Both "ancient philosophy" and "philosophy" match; the longer one has
        // to win or every period files under the parent.
        assert_eq!(
            shelf().file_one("Ancient philosophy—History"),
            Some("Ancient Philosophy")
        );
    }

    #[test]
    fn real_cataloguing_headings_file() {
        let s = shelf();
        for (raw, want) in [
            (
                "Political science – Early works to 1800",
                "Political Science",
            ),
            ("Political science — Philosophy", "Political Science"),
            ("Byzantine Empire—History—527-1081", "Byzantine History"),
            ("Christianity and culture", "Theology"),
        ] {
            assert_eq!(s.file_one(raw), Some(want), "{raw}");
        }
    }

    #[test]
    fn a_dash_is_a_dash_however_it_was_printed() {
        // em, en, hyphen, and the double hyphen OCR leaves behind.
        let s = shelf();
        for raw in [
            "Philosophy—Introductions",
            "Philosophy – Introductions",
            "Philosophy - Introductions",
            "Philosophy -- Introductions",
            "Philosophy. Introductions",
        ] {
            assert_eq!(s.file_one(raw), Some("Philosophy"), "{raw}");
        }
    }

    #[test]
    fn a_term_must_match_a_whole_word() {
        // "politics" must not file a book on "geopolitics", and "art" must not
        // file one on cartography.
        let s = Shelf::parse("Art\nPolitical Science = politics\n");
        assert_eq!(s.file_one("Cartography—History"), None);
        assert_eq!(s.file_one("Art—Byzantine"), Some("Art"));
    }

    #[test]
    fn what_matches_nothing_is_reported_rather_than_dropped() {
        let filed = shelf().file(&[
            "Political science – Early works to 1800".into(),
            "Selectivity (Psychology)".into(),
        ]);
        assert_eq!(filed.on, ["Political Science"]);
        assert_eq!(filed.unfiled, ["Selectivity (Psychology)"]);
    }

    #[test]
    fn a_heading_is_not_repeated() {
        let filed = shelf().file(&["Philosophy—Ancient".into(), "Philosophy—Modern".into()]);
        assert_eq!(filed.on, ["Philosophy"]);
    }

    #[test]
    fn an_empty_shelf_files_nothing_and_says_so() {
        let s = Shelf::parse("   \n# nothing but a comment\n");
        assert!(s.is_empty());
        assert_eq!(s.file(&["Philosophy".into()]).unfiled, ["Philosophy"]);
    }
}
