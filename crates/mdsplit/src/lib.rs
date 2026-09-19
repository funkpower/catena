//! Splits one markdown file into the works inside it.
//!
//! A collected edition is one file holding forty books; a Delphi anthology is
//! one file holding a hundred. Indexed whole, a query for one of them
//! retrieves a passage that could be from any of them, and the title on the
//! result names the collection rather than the work.
//!
//! The split is by heading level, with a gate on whether the result is
//! plausible — and scripture gets its own rule, because every chapter there
//! is its own heading and the ordinary split would give a thousand fragments
//! rather than the seventy-odd books anybody wants.

pub mod bible;
mod books;

use std::sync::LazyLock;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use textkit::pattern::SPACE;
use unicode_normalization::UnicodeNormalization;

/// Fewer sections than this and the document was not a collection.
pub const MIN_SECTIONS: usize = 5;

/// More than this and the split found structure that is not there.
pub const MAX_SECTIONS: usize = 1500;

/// The typical section must be at least this long, or the split is cutting
/// at the line level rather than at the work level.
pub const MIN_MEDIAN_CHARS: usize = 1250;

/// A section this short is a heading with no body under it.
///
/// Anthologies emit them in pairs — `# CHAPTER I` immediately followed by
/// `# THE WORK` — and the second is the one that owns the content.
pub const STUB_MAX_CHARS: usize = 200;

/// How long a section's identifier may be.
const SLUG_MAX: usize = 80;

/// A section that comes after the numbered books, sorted by where it appeared.
const AFTER_THE_CANON: usize = 10_000;

static SPACE_RUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"{SPACE}+")).expect("a valid pattern"));

static NOT_SLUG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^a-zA-Z0-9]+").expect("a valid pattern"));

/// One work out of a collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Section {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// Position in the collection, from zero.
    pub idx: u32,
    /// The heading it was found under.
    pub title: String,
    /// An identifier made from the title.
    pub slug: String,
    /// The section, heading and all, ready to be written to a file.
    pub text: String,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Section {
    fn version(&self) -> u32 {
        self.v
    }
}

/// Why a document could not be split.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct Unsplittable(pub String);

/// Split a document into the works inside it.
///
/// Scripture first, then top-level headings, then the level below. Each
/// candidate has to look like a collection before it is accepted; a document
/// that is simply one work comes back as an error rather than as one section,
/// because "this is not a collection" is the useful answer.
pub fn split(markdown: &str) -> Result<Vec<Section>, Unsplittable> {
    let headings = headings_at(markdown, 1);
    if bible::looks_like_scripture(&headings) {
        let sections = by_book(markdown);
        if implausible(&sections).is_none() {
            return Ok(numbered(sections));
        }
        // Fall through: a short document that merely mentions a few books.
    }

    let first = by_heading_level(markdown, 1);
    let Some(why_first) = implausible(&first) else {
        return Ok(numbered(first));
    };
    let second = by_heading_level(markdown, 2);
    let Some(why_second) = implausible(&second) else {
        return Ok(numbered(second));
    };

    Err(Unsplittable(format!(
        "one heading level gave {} sections ({why_first}); the next gave {} ({why_second})",
        first.len(),
        second.len()
    )))
}

/// Why a set of sections is not a plausible split, or `None` if it is.
///
/// Permissive on purpose. Collections vary wildly — one giant work among
/// sixty ordinary ones, five hundred short homilies, a hundred and eighty
/// distinct works — so only the pathological shapes are refused: no structure
/// at all, and structure found line by line.
pub fn implausible(sections: &[Section]) -> Option<String> {
    if sections.len() < MIN_SECTIONS {
        return Some(format!(
            "only {} sections, fewer than {MIN_SECTIONS}",
            sections.len()
        ));
    }
    if sections.len() > MAX_SECTIONS {
        return Some(format!(
            "{} sections, more than {MAX_SECTIONS} — over-fragmented",
            sections.len()
        ));
    }
    // The median, because one enormous section among many ordinary ones is a
    // real shape and an average would hide it.
    let mut lengths: Vec<usize> = sections.iter().map(|s| s.text.len()).collect();
    lengths.sort_unstable();
    let median = lengths[lengths.len() / 2];
    if median < MIN_MEDIAN_CHARS {
        return Some(format!(
            "the typical section is {median} characters, under {MIN_MEDIAN_CHARS} — too granular"
        ));
    }
    None
}

/// Every heading at one level, in order.
pub fn headings_at(markdown: &str, level: usize) -> Vec<&str> {
    heading_pattern(level)
        .captures_iter(markdown)
        .map(|found| found.get(1).expect("a heading").as_str())
        .collect()
}

fn heading_pattern(level: usize) -> Regex {
    Regex::new(&format!(
        r"(?m)^{}{SPACE}+(.+?){SPACE}*$",
        "#".repeat(level)
    ))
    .expect("a valid pattern")
}

/// Carve a document at every heading of exactly this level.
///
/// A section runs from one heading to the next at the same level. Whatever
/// sits above the first heading — a contents page, an acknowledgement — goes
/// onto the first section rather than being dropped, because it is usually
/// the only place the collection describes itself.
pub fn by_heading_level(markdown: &str, level: usize) -> Vec<Section> {
    let pattern = heading_pattern(level);
    let found: Vec<_> = pattern.captures_iter(markdown).collect();
    let Some(first) = found.first() else {
        return Vec::new();
    };

    let frontmatter = &markdown[..first.get(0).expect("a match").start()];
    let mut raw: Vec<(String, String)> = Vec::new();
    for (at, heading) in found.iter().enumerate() {
        let whole = heading.get(0).expect("a match");
        let end = found
            .get(at + 1)
            .map_or(markdown.len(), |next| next.get(0).expect("a match").start());
        let mut body = markdown[whole.start()..end].to_string();
        if at == 0 && !frontmatter.trim_matches(textkit::is_space).is_empty() {
            body = format!(
                "{}\n\n{body}",
                frontmatter.trim_end_matches(textkit::is_space)
            );
        }
        raw.push((
            tidy(heading.get(1).expect("a title").as_str()),
            format!("{}\n", body.trim_end_matches(textkit::is_space)),
        ));
    }

    merge_stubs(raw)
}

/// Fold heading-only sections into the one that follows, keeping their text
/// so the heading is still searchable as context.
fn merge_stubs(raw: Vec<(String, String)>) -> Vec<Section> {
    let mut merged: Vec<(String, String)> = Vec::new();
    let mut pending: Vec<String> = Vec::new();

    for (title, body) in raw {
        if body.len() <= STUB_MAX_CHARS {
            pending.push(body);
            continue;
        }
        let body = match pending.is_empty() {
            true => body,
            false => format!("{}{body}", std::mem::take(&mut pending).concat()),
        };
        merged.push((title, body));
    }
    // Stubs after the last real section just hang off the end of it.
    if !pending.is_empty()
        && let Some(last) = merged.last_mut()
    {
        last.1.push_str(&pending.concat());
    }

    merged
        .into_iter()
        .enumerate()
        .map(|(at, (title, text))| section(at, title, text))
        .collect()
}

/// Group a scripture document's chapters into one section per book.
///
/// A heading that names no book — a preface, a map, a table of weights —
/// becomes a section of its own and sorts after the canon, so the books
/// themselves come out in canonical order whatever order the edition printed
/// them in.
pub fn by_book(markdown: &str) -> Vec<Section> {
    let pattern = heading_pattern(1);
    let found: Vec<_> = pattern.captures_iter(markdown).collect();
    let Some(first) = found.first() else {
        return Vec::new();
    };

    let mut sections: Vec<Section> = Vec::new();
    let mut where_book_went: Vec<(&'static str, usize)> = Vec::new();
    let mut open: Option<(&'static str, usize, Vec<String>)> = None;
    let mut pending_frontmatter = {
        let frontmatter = &markdown[..first.get(0).expect("a match").start()];
        match frontmatter.trim_matches(textkit::is_space).is_empty() {
            true => String::new(),
            false => format!("{}\n\n", frontmatter.trim_end_matches(textkit::is_space)),
        }
    };

    for (at, heading) in found.iter().enumerate() {
        let whole = heading.get(0).expect("a match");
        let title = heading.get(1).expect("a title").as_str();
        let end = found
            .get(at + 1)
            .map_or(markdown.len(), |next| next.get(0).expect("a match").start());
        let body = &markdown[whole.start()..end];

        match bible::classify(title) {
            None => {
                close(&mut sections, &mut where_book_went, open.take());
                let body = format!("{}{body}", std::mem::take(&mut pending_frontmatter));
                sections.push(section(
                    AFTER_THE_CANON + sections.len(),
                    tidy(title),
                    format!("{}\n", body.trim_end_matches(textkit::is_space)),
                ));
            }
            Some((canonical, ordinal)) => {
                if open.as_ref().map(|(book, _, _)| *book) != Some(canonical) {
                    close(&mut sections, &mut where_book_went, open.take());
                    let mut buffer = Vec::new();
                    if !pending_frontmatter.is_empty() {
                        buffer.push(std::mem::take(&mut pending_frontmatter));
                    }
                    open = Some((canonical, ordinal, buffer));
                }
                if let Some((_, _, buffer)) = open.as_mut() {
                    buffer.push(body.to_string());
                }
            }
        }
    }
    close(&mut sections, &mut where_book_went, open.take());

    sections.sort_by_key(|section| section.idx);
    sections
}

/// Finish an open book, merging it into the section already emitted for it
/// when the edition interrupted itself.
///
/// Editions do interrupt themselves: a prologue between a book's
/// introduction and its first chapter would otherwise make the book appear
/// twice, as two separate works.
fn close(
    sections: &mut Vec<Section>,
    placed: &mut Vec<(&'static str, usize)>,
    open: Option<(&'static str, usize, Vec<String>)>,
) {
    let Some((book, ordinal, buffer)) = open else {
        return;
    };
    if buffer.is_empty() {
        return;
    }
    let text = format!("{}\n", buffer.concat().trim_end_matches(textkit::is_space));

    match placed.iter().find(|(name, _)| *name == book) {
        Some((_, at)) => {
            let prior = &mut sections[*at];
            prior.text = format!(
                "{}\n\n{text}",
                prior.text.trim_end_matches(textkit::is_space)
            );
        }
        None => {
            placed.push((book, sections.len()));
            sections.push(section(ordinal, book.to_string(), text));
        }
    }
}

/// Renumber sections so they come out in the order they will be read.
fn numbered(sections: Vec<Section>) -> Vec<Section> {
    sections
        .into_iter()
        .enumerate()
        .map(|(at, section)| Section {
            idx: at as u32,
            ..section
        })
        .collect()
}

fn section(idx: usize, title: String, text: String) -> Section {
    Section {
        v: corpus_core::V,
        idx: idx as u32,
        slug: slugify(&title),
        title,
        text,
    }
}

fn tidy(title: &str) -> String {
    SPACE_RUN
        .replace_all(title.trim_matches(textkit::is_space), " ")
        .to_string()
}

/// An identifier made from a title.
///
/// Anything that is not a letter or a digit becomes a separator, and anything
/// that is not ASCII is dropped rather than transliterated — the identifier
/// only has to be stable and readable, and a title in another script still
/// has its own `title` field.
pub fn slugify(title: &str) -> String {
    let folded: String = title.nfkd().filter(char::is_ascii).collect();
    let slug = NOT_SLUG
        .replace_all(&folded, "-")
        .trim_matches('-')
        .to_lowercase();
    let mut slug: String = slug.chars().take(SLUG_MAX).collect();
    if slug.is_empty() {
        slug.push_str("section");
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anthology(count: usize) -> String {
        (1..=count)
            .map(|n| format!("# Work {n}\n\n{}\n\n", "body text ".repeat(200)))
            .collect()
    }

    #[test]
    fn a_collection_splits_at_its_headings() {
        let sections = split(&anthology(8)).expect("a split");
        assert_eq!(sections.len(), 8);
        assert_eq!(sections[0].title, "Work 1");
        assert_eq!(sections[0].slug, "work-1");
        assert_eq!(sections[0].idx, 0);
        assert!(sections[0].text.starts_with("# Work 1"));
    }

    #[test]
    fn a_single_work_is_not_a_collection() {
        let one = format!("# The Politics\n\n{}", "body ".repeat(2000));
        assert!(split(&one).is_err());
    }

    #[test]
    fn what_sits_above_the_first_heading_is_kept() {
        let markdown = format!("A table of contents\n\n{}", anthology(6));
        let sections = split(&markdown).expect("a split");
        assert!(
            sections[0].text.contains("A table of contents"),
            "{:?}",
            sections[0].text
        );
    }

    #[test]
    fn a_heading_with_nothing_under_it_folds_into_the_next() {
        let markdown = format!(
            "# CHAPTER I\n\n# The Real Work\n\n{}\n\n{}",
            "body ".repeat(200),
            anthology(6)
        );
        let sections = split(&markdown).expect("a split");
        assert_eq!(sections[0].title, "The Real Work");
        assert!(
            sections[0].text.contains("CHAPTER I"),
            "the stub's own text is kept as context"
        );
    }

    #[test]
    fn the_level_below_is_tried_when_the_top_one_says_nothing() {
        let markdown = format!(
            "# The Collected Works\n\n{}",
            (1..=7)
                .map(|n| format!("## Work {n}\n\n{}\n\n", "body text ".repeat(200)))
                .collect::<String>()
        );
        let sections = split(&markdown).expect("a split");
        assert_eq!(sections.len(), 7);
        assert_eq!(sections[0].title, "Work 1");
    }

    #[test]
    fn scripture_is_grouped_by_book_rather_than_by_chapter() {
        let mut markdown = String::new();
        let books = [
            "Genesis",
            "Exodus",
            "Leviticus",
            "Numbers",
            "Deuteronomy",
            "Joshua",
        ];
        for book in books {
            for chapter in 1..=15 {
                markdown.push_str(&format!(
                    "# {book} {chapter}\n\n{}\n\n",
                    "verse text ".repeat(40)
                ));
            }
        }
        let sections = split(&markdown).expect("a split");
        assert_eq!(sections.len(), 6, "one section per book, not per chapter");
        assert_eq!(sections[0].title, "Genesis");
        assert!(sections[0].text.contains("# Genesis 15"));
    }

    #[test]
    fn a_book_the_edition_interrupts_is_still_one_section() {
        let mut markdown = String::new();
        markdown.push_str(&format!(
            "# The Book of Genesis\n\n{}\n\n",
            "intro ".repeat(80)
        ));
        markdown.push_str(&format!("# The Prologue\n\n{}\n\n", "prologue ".repeat(80)));
        for chapter in 1..=40 {
            markdown.push_str(&format!(
                "# Genesis {chapter}\n\n{}\n\n",
                "verse text ".repeat(40)
            ));
        }
        for book in ["Exodus", "Leviticus", "Numbers", "Deuteronomy"] {
            for chapter in 1..=10 {
                markdown.push_str(&format!(
                    "# {book} {chapter}\n\n{}\n\n",
                    "verse text ".repeat(40)
                ));
            }
        }
        let sections = split(&markdown).expect("a split");
        let genesis: Vec<&Section> = sections.iter().filter(|s| s.title == "Genesis").collect();
        assert_eq!(
            genesis.len(),
            1,
            "{:?}",
            sections.iter().map(|s| &s.title).collect::<Vec<_>>()
        );
        assert!(genesis[0].text.contains("The Book of Genesis"));
        assert!(genesis[0].text.contains("# Genesis 40"));
    }

    #[test]
    fn scripture_comes_out_in_canonical_order() {
        let mut markdown = String::new();
        for book in [
            "Revelation",
            "Genesis",
            "Exodus",
            "Leviticus",
            "Numbers",
            "Joshua",
        ] {
            for chapter in 1..=12 {
                markdown.push_str(&format!(
                    "# {book} {chapter}\n\n{}\n\n",
                    "verse text ".repeat(40)
                ));
            }
        }
        let sections = split(&markdown).expect("a split");
        let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Genesis",
                "Exodus",
                "Leviticus",
                "Numbers",
                "Joshua",
                "Revelation"
            ]
        );
    }

    #[test]
    fn a_heading_that_is_not_scripture_sorts_after_the_books() {
        let mut markdown = String::new();
        for chapter in 1..=40 {
            markdown.push_str(&format!(
                "# Genesis {chapter}\n\n{}\n\n",
                "verse ".repeat(60)
            ));
        }
        markdown.push_str(&format!(
            "# Table of Weights\n\n{}\n\n",
            "measures ".repeat(80)
        ));
        for book in ["Exodus", "Leviticus", "Numbers", "Deuteronomy"] {
            for chapter in 1..=10 {
                markdown.push_str(&format!(
                    "# {book} {chapter}\n\n{}\n\n",
                    "verse ".repeat(60)
                ));
            }
        }
        let sections = split(&markdown).expect("a split");
        let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Genesis",
                "Exodus",
                "Leviticus",
                "Numbers",
                "Deuteronomy",
                "Table of Weights"
            ]
        );
    }

    #[test]
    fn a_title_in_another_script_still_gets_an_identifier() {
        assert_eq!(slugify("Ἀθῆναι"), "section");
        assert_eq!(slugify("Être et Temps"), "etre-et-temps");
        assert_eq!(slugify("The Politics!"), "the-politics");
        assert_eq!(slugify(""), "section");
    }

    #[test]
    fn an_identifier_is_not_unbounded() {
        assert!(slugify(&"word ".repeat(100)).chars().count() <= SLUG_MAX);
    }

    #[test]
    fn nothing_in_is_not_splittable() {
        assert!(split("").is_err());
    }
}
