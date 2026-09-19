//! Bibliographic hygiene over catalogue records.
//!
//! A library's metadata rots in recognisable ways: a title that is really a
//! filename, an author field holding a publisher, a scanner's name where the
//! work's name should be, capitals shouted by a converter. None of it stops
//! anything working, and all of it makes a catalogue harder to search than
//! the shelf it describes.
//!
//! Nothing here is rewritten silently. Every change comes back with the
//! reason it was made, so a person can read a run before agreeing to it —
//! which is the difference between a tool that cleans a catalogue and one
//! that damages it in a new way.

pub mod fix;
pub mod isbn;
pub mod matching;
pub mod reasons;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A catalogue record, as it arrives.
///
/// Anything this does not know about is carried through untouched, so a
/// record can be piped through and back into whatever it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Book {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// The work's title.
    #[serde(default)]
    pub title: Option<String>,
    /// Its author, as one field, however many there are.
    #[serde(default)]
    pub author: Option<String>,
    /// Its ISBN, if the catalogue has one.
    #[serde(default)]
    pub isbn: Option<String>,
    /// Where the file is, which is often where a missing ISBN is hiding.
    #[serde(default)]
    pub path: Option<String>,
    /// Everything else the record carried.
    #[serde(flatten)]
    #[schemars(with = "Map<String, Value>")]
    pub extra: Map<String, Value>,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Book {
    fn version(&self) -> u32 {
        self.v
    }
}

/// One change, and why it was made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Change {
    /// Which field changed.
    pub field: String,
    /// What it was.
    pub from: String,
    /// What it became.
    pub to: String,
    /// The fault that prompted it.
    pub why: String,
}

/// A record, cleaned, with everything that was found and done to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cleaned {
    /// The record, with the changes applied.
    #[serde(flatten)]
    pub book: Book,
    /// What is wrong with it, named.
    pub reasons: Vec<String>,
    /// How much attention it deserves, worst first.
    pub severity: u32,
    /// What was changed, and why.
    pub changes: Vec<Change>,
}

/// Audit a record and make the repairs that need no lookup.
pub fn clean(book: &Book) -> Cleaned {
    let mut cleaned = book.clone();
    let mut changes = Vec::new();

    let reasons = reasons::problems(
        book.title.as_deref(),
        book.author.as_deref(),
        book.isbn.as_deref(),
    );

    // Underscores first: a title mangled by a filename may also be shouting,
    // and title-casing it before the underscores come out would case the
    // format suffix as though it were a word.
    if let Some(title) = cleaned.title.clone()
        && reasons.iter().any(|reason| reason == "underscore_title")
    {
        let fixed = fix::clean_underscores(&title);
        if fixed != title {
            changes.push(Change {
                field: "title".into(),
                from: title,
                to: fixed.clone(),
                why: "underscore_title".into(),
            });
            cleaned.title = Some(fixed);
        }
    }

    if let Some(title) = cleaned.title.clone()
        && fix::is_shouting(&title)
    {
        let fixed = fix::title_case(&title);
        if fixed != title {
            changes.push(Change {
                field: "title".into(),
                from: title,
                to: fixed.clone(),
                why: "allcaps_title".into(),
            });
            cleaned.title = Some(fixed);
        }
    }

    // A missing ISBN is often sitting in the filename, where it is free and
    // certain — the checksum is what makes it certain.
    if reasons.iter().any(|reason| reason == "missing_isbn")
        && let Some(path) = book.path.as_deref()
        && let Some(found) = isbn::from_filename(path)
    {
        changes.push(Change {
            field: "isbn".into(),
            from: book.isbn.clone().unwrap_or_default(),
            to: found.clone(),
            why: "missing_isbn".into(),
        });
        cleaned.isbn = Some(found);
    }

    Cleaned {
        severity: reasons::severity(&reasons),
        reasons,
        changes,
        book: cleaned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(title: &str, author: &str) -> Book {
        Book {
            v: corpus_core::V,
            title: Some(title.into()),
            author: Some(author.into()),
            isbn: None,
            path: None,
            extra: Map::new(),
        }
    }

    #[test]
    fn a_good_record_comes_back_unchanged() {
        let book = Book {
            isbn: Some("9780198237907".into()),
            ..book("The Human Condition", "Arendt, Hannah")
        };
        let cleaned = clean(&book);
        assert_eq!(cleaned.book, book);
        assert!(cleaned.reasons.is_empty());
        assert!(cleaned.changes.is_empty());
        assert_eq!(cleaned.severity, 0);
    }

    #[test]
    fn a_shouting_title_is_cased_and_the_change_is_recorded() {
        let cleaned = clean(&book("THE HUMAN CONDITION", "Arendt, Hannah"));
        assert_eq!(cleaned.book.title.as_deref(), Some("The Human Condition"));
        assert_eq!(cleaned.changes.len(), 1);
        assert_eq!(cleaned.changes[0].field, "title");
        assert_eq!(cleaned.changes[0].why, "allcaps_title");
        assert_eq!(cleaned.changes[0].from, "THE HUMAN CONDITION");
    }

    #[test]
    fn an_underscored_title_is_cleaned_before_it_is_cased() {
        let cleaned = clean(&book("THE FALL_ AND THE RISE_txt", "Arendt, Hannah"));
        assert_eq!(
            cleaned.book.title.as_deref(),
            Some("The Fall: and the Rise")
        );
        assert_eq!(cleaned.changes.len(), 2);
    }

    #[test]
    fn an_isbn_in_the_filename_fills_a_missing_one() {
        let mut book = book("The Human Condition", "Arendt, Hannah");
        book.path = Some("/library/9780198237907.pdf".into());
        let cleaned = clean(&book);
        assert_eq!(cleaned.book.isbn.as_deref(), Some("9780198237907"));
        assert_eq!(cleaned.changes[0].why, "missing_isbn");
    }

    #[test]
    fn a_filename_that_is_not_an_isbn_fills_nothing() {
        let mut book = book("The Human Condition", "Arendt, Hannah");
        book.path = Some("/library/20231115123456.pdf".into());
        assert_eq!(clean(&book).book.isbn, None);
    }

    #[test]
    fn fields_it_does_not_know_about_are_carried_through() {
        let mut book = book("A Book", "An Author");
        book.extra.insert("shelf".into(), Value::from("upstairs"));
        let cleaned = clean(&book);
        assert_eq!(cleaned.book.extra["shelf"], "upstairs");
    }

    #[test]
    fn the_output_carries_the_record_and_the_audit_on_one_line() {
        let cleaned = clean(&book("THE HUMAN CONDITION", "Arendt, Hannah"));
        let json: Value = serde_json::to_value(&cleaned).expect("a record serialises");
        assert_eq!(json["title"], "The Human Condition");
        assert!(json["reasons"].as_array().is_some());
        assert!(json["changes"].as_array().is_some());
    }
}
