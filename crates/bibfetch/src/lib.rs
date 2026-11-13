//! What a catalogue says about an identifier.
//!
//! [`idnorm`] can tell you a DOI is well formed. It cannot tell you the paper
//! exists, because that is not a fact about the string — it is a fact about
//! the world, and the only way to learn it is to ask somebody who keeps a
//! register.
//!
//! So this asks. It is the only thing it does, which is the condition under
//! which a filter here is allowed to touch the network at all.
//!
//! Three registers, one per kind of identifier, each the body that assigns
//! them: Crossref for a DOI, the arXiv for a preprint, OpenLibrary for a
//! book. There is no register for an ISSN that answers without a key, so an
//! ISSN is looked up as the journal of a work rather than on its own.
//!
//! A book with no identifier at all can still be looked for by its title,
//! with [`search()`]: Open Library's index, then Google Books, each candidate
//! a record of one edition, in the order the register gave them.

pub mod ask;
pub mod cover;
pub mod search;
pub mod source;

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use cover::{Cover, CoverSize, cover};
pub use search::search;
pub use source::Source;

/// How long to wait on a register before giving up.
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// How many times to ask before believing the answer.
pub const ATTEMPTS: u32 = 3;

/// What a register said about one identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Record {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// The identifier asked about, normalised.
    pub id: String,
    /// Which kind it is.
    pub kind: String,
    /// Whether the register has it. This is the answer the string alone
    /// cannot give.
    pub found: bool,
    /// Which register answered.
    pub source: String,
    /// The title, where there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The authors, in the order the register lists them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// The year of publication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i64>,
    /// The journal, publisher or archive it appeared under.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    /// The abstract, where the register carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The subject headings the register files it under, as it words them.
    ///
    /// A register states these for far more books than print a cataloguing
    /// block, which makes them the second source a shelf otherwise lacks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
    /// The language it is written in, as the register gives it: a two- or
    /// three-letter code, or a name. Not normalised here — what a register
    /// said is the record, and deciding that "eng" and "English" are one
    /// thing belongs to whoever is filing them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// How many pages the register says it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<u32>,
    /// The first cover id in Open Library's record of this edition, for
    /// [`cover()`] to ask by when the ISBN has none. Taken from the edition
    /// record only, never from the index's cover for the work, which is
    /// often another edition's; `None` when the edition names no cover or
    /// the lookup stopped before asking for the edition record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_id: Option<u64>,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Record {
    fn version(&self) -> u32 {
        self.v
    }
}

/// What can go wrong asking.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The register could not be reached, or refused.
    Unreachable(String),
    /// It answered with something this could not read.
    Shape(String),
    /// Nothing here knows where to ask about this kind.
    NoRegister(String),
    /// What was asked about is not an identifier of the kind it should be.
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unreachable(why) => write!(f, "{why}"),
            Error::Shape(why) => write!(f, "the answer could not be read: {why}"),
            Error::NoRegister(kind) => {
                write!(f, "no register here answers about {kind} identifiers")
            }
            Error::Invalid(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

/// A record for an identifier the register does not have.
///
/// Absence is an answer, and a useful one: it is what tells a catalogue that
/// a well-formed DOI it has been carrying for three years was mistyped.
pub fn missing(id: &str, kind: &str, source: Source) -> Record {
    Record {
        v: corpus_core::V,
        id: id.to_string(),
        kind: kind.to_string(),
        found: false,
        source: source.name().to_string(),
        title: None,
        authors: Vec::new(),
        year: None,
        container: None,
        summary: None,
        subjects: Vec::new(),
        language: None,
        pages: None,
        cover_id: None,
    }
}

impl Record {
    /// Whether this record says everything a catalogue wants.
    ///
    /// Used to decide when to stop asking. Title and authors are the minimum
    /// that makes a record usable; the rest is worth another register only
    /// while one is left to ask.
    pub fn complete(&self) -> bool {
        self.found
            && self.title.is_some()
            && !self.authors.is_empty()
            && self.container.is_some()
            && self.language.is_some()
    }

    /// Take from another record whatever this one does not have.
    ///
    /// The registers are complementary rather than redundant, and asking them
    /// in turn and keeping one answer throws away the difference. OpenLibrary's
    /// edition record gives the publisher, the pagination and the language but
    /// names its authors by key; its search index gives the authors but not the
    /// publisher. One book, two half-answers, and the whole of it only if they
    /// are put together.
    ///
    /// Nothing already answered is overwritten, so the order of the chain is
    /// the order of trust.
    pub fn fill_from(&mut self, other: Record) {
        if !other.found {
            return;
        }
        self.found = true;
        if self.title.is_none() {
            self.title = other.title;
        }
        if self.authors.is_empty() {
            self.authors = other.authors;
        }
        if self.year.is_none() {
            self.year = other.year;
        }
        if self.container.is_none() {
            self.container = other.container;
        }
        if self.summary.is_none() {
            self.summary = other.summary;
        }
        if self.subjects.is_empty() {
            self.subjects = other.subjects;
        }
        if self.language.is_none() {
            self.language = other.language;
        }
        if self.pages.is_none() {
            self.pages = other.pages;
        }
        if self.cover_id.is_none() {
            self.cover_id = other.cover_id;
        }
        // Which register answered becomes which registers did.
        if !self.source.split('+').any(|s| s == other.source) {
            self.source = format!("{}+{}", self.source, other.source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identifier_nobody_has_is_still_an_answer() {
        let record = missing("10.1000/nope", "doi", Source::Crossref);
        assert!(!record.found);
        assert_eq!(record.source, "crossref");
        assert_eq!(record.title, None);
    }

    #[test]
    fn a_record_that_found_nothing_serialises_without_empty_fields() {
        let json = serde_json::to_string(&missing("x", "doi", Source::Crossref)).expect("json");
        assert!(!json.contains("title"), "{json}");
        assert!(!json.contains("authors"), "{json}");
        assert!(json.contains("\"found\":false"), "{json}");
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    fn rec(source: &str) -> Record {
        let mut r = missing("9780199205592", "isbn", Source::OpenLibrary);
        r.source = source.to_string();
        r.found = true;
        r
    }

    #[test]
    fn two_half_answers_make_one_record() {
        // Exactly what the two OpenLibrary endpoints give for one book: the
        // edition record has the publisher and the language and names its
        // authors by key; the index has the authors and not the publisher.
        let mut edition = rec("openlibrary");
        edition.title = Some("The First World War: A Very Short Introduction".into());
        edition.container = Some("Oxford University Press".into());
        edition.language = Some("eng".into());

        let mut index = rec("openlibrary");
        index.title = Some("The First World War".into());
        index.authors = vec!["Michael Howard".into()];
        index.year = Some(2007);

        edition.fill_from(index);
        assert_eq!(edition.authors, ["Michael Howard"]);
        assert_eq!(edition.year, Some(2007));
        // The fuller title arrived first and is not displaced by the shorter.
        assert_eq!(
            edition.title.as_deref(),
            Some("The First World War: A Very Short Introduction")
        );
    }

    #[test]
    fn a_register_that_has_nothing_contributes_nothing() {
        let mut kept = rec("openlibrary");
        kept.title = Some("A Book".into());
        let absent = missing("9780199205592", "isbn", Source::GoogleBooks);
        kept.fill_from(absent);
        assert_eq!(kept.title.as_deref(), Some("A Book"));
        assert_eq!(kept.source, "openlibrary");
    }

    #[test]
    fn the_record_names_every_register_that_answered() {
        let mut a = rec("openlibrary");
        a.title = Some("A Book".into());
        let mut b = rec("googlebooks");
        b.found = true;
        b.pages = Some(134);
        a.fill_from(b);
        assert_eq!(a.source, "openlibrary+googlebooks");
        assert_eq!(a.pages, Some(134));
    }

    #[test]
    fn complete_means_a_catalogue_has_what_it_needs() {
        let mut r = rec("openlibrary");
        assert!(!r.complete());
        r.title = Some("A Book".into());
        r.authors = vec!["Someone".into()];
        assert!(!r.complete(), "still missing the publisher and language");
        r.container = Some("A Press".into());
        r.language = Some("eng".into());
        assert!(r.complete());
    }
}
