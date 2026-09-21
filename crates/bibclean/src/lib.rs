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

pub mod abstracts;
pub mod author;
mod entities4;
pub mod fix;
pub mod isbn;
pub mod matching;
pub mod reasons;
pub mod scheme;
pub mod slug;
pub mod vocab;

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
    /// The public identifier the record already has, if it has one.
    #[serde(default)]
    pub slug: Option<String>,
    /// A number that makes a slug unique. Needed to judge or regenerate one.
    #[serde(default)]
    pub id: Option<i64>,
    /// The opening pages of the work, if the caller has them.
    ///
    /// A book says who wrote it on its own copyright page, which is the
    /// cheapest and most trustworthy answer there is for a record whose
    /// author is missing.
    ///
    /// Read but never written back: it is evidence handed in, not part of
    /// the record, and several thousand characters of it on every line would
    /// bury everything else.
    #[serde(default, skip_serializing)]
    pub front_matter: Option<String>,
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
    /// A name for a record that has none, with the evidence for it.
    ///
    /// Proposed rather than applied, because the evidence differs in strength
    /// and a wrong name is worse than an honest gap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_proposal: Option<author::Suggestion>,
    /// What should happen to the record's slug, when it has one to judge.
    ///
    /// Named apart from the record's own `slug`, which is carried through
    /// beside it — the two would otherwise be one field and the proposal
    /// would quietly replace the fact.
    ///
    /// Never applied as a change: a slug is a public identifier that other
    /// things have already written down, so moving one is a decision with
    /// consequences outside this record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug_proposal: Option<slug::Proposal>,
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

    // Judged against the record as it arrived, not as it was repaired: the
    // question is whether the slug describes the book, and a title this just
    // title-cased describes the same book either way.
    let proposal = match (book.slug.as_deref(), book.id) {
        (Some(slug), Some(id)) => {
            slug::evaluate(slug, book.author.as_deref(), book.title.as_deref(), id)
        }
        _ => None,
    };

    // Only for a record that has no author to overwrite. A name recovered
    // from the text is evidence, not an improvement on one somebody entered.
    let author_proposal = reasons
        .iter()
        .any(|reason| reason == "unknown_author" || reason == "placeholder_author")
        .then(|| {
            author::best(
                book.title.as_deref().unwrap_or(""),
                book.front_matter.as_deref(),
            )
        })
        .flatten();

    Cleaned {
        severity: reasons::severity(&reasons),
        reasons,
        changes,
        author_proposal,
        slug_proposal: proposal,
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
            slug: None,
            id: None,
            front_matter: None,
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
        // "And" is capitalised because the colon starts a subtitle, and the
        // first word of one is capitalised however small it is. This asserted
        // "and" until the rule was found to be counting small words from the
        // start of the whole string, which set every subtitle opening "A",
        // "The" or "An" in lower case.
        assert_eq!(
            cleaned.book.title.as_deref(),
            Some("The Fall: And the Rise")
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
    fn a_record_with_no_author_gets_one_proposed_from_its_own_pages() {
        let mut book = book("The Icon and the Axe", "Unknown");
        book.front_matter = Some("Copyright © 1966 by James H. Billington".into());

        let cleaned = clean(&book);
        let proposal = cleaned.author_proposal.expect("a proposal");
        assert_eq!(proposal.author, "James H. Billington");
        assert_eq!(proposal.method, "front_matter_copyright");
        // Proposed, never applied.
        assert_eq!(cleaned.book.author.as_deref(), Some("Unknown"));
    }

    #[test]
    fn a_record_that_has_an_author_is_not_second_guessed() {
        let mut book = book("The Icon and the Axe", "Billington, James H.");
        book.front_matter = Some("Copyright © 1966 by Somebody Else".into());
        assert!(clean(&book).author_proposal.is_none());
    }

    #[test]
    fn a_slug_is_judged_when_there_is_one_to_judge() {
        let mut book = book("Weaponizing Anthropology", "Laura Nader");
        book.slug = Some("william-zinsser-on-writing-well-3450".into());
        book.id = Some(3450);

        let cleaned = clean(&book);
        let proposal = cleaned.slug_proposal.expect("a proposal");
        assert_eq!(proposal.tier, "wrong-book");
        assert_eq!(
            proposal.proposed,
            "laura-nader-weaponizing-anthropology-3450"
        );
        // Proposed, never applied.
        assert_eq!(
            cleaned.book.slug.as_deref(),
            Some("william-zinsser-on-writing-well-3450")
        );
    }

    #[test]
    fn the_record_keeps_its_own_slug_beside_the_proposal() {
        let mut book = book("Weaponizing Anthropology", "Laura Nader");
        book.slug = Some("william-zinsser-on-writing-well-3450".into());
        book.id = Some(3450);

        let json = serde_json::to_value(clean(&book)).expect("a record serialises");
        assert_eq!(json["slug"], "william-zinsser-on-writing-well-3450");
        assert_eq!(json["slug_proposal"]["tier"], "wrong-book");
    }

    #[test]
    fn a_record_with_no_slug_gets_no_proposal() {
        assert!(clean(&book("A Book", "An Author")).slug_proposal.is_none());
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
