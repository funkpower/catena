//! The references inside a bibliography or a page of notes.
//!
//! A reference list is the one place in a book where the text is already
//! structured — every entry names a work, and most of them name it in one of
//! a handful of shapes that have not changed in a century. What is missing is
//! not information but punctuation: the entries run together, and nothing in
//! the file says where one stops.
//!
//! So this does two jobs. It cuts the block into entries, which is mechanical
//! and gets it right or visibly wrong. Then it reads each entry as far as the
//! shape allows, and says which fields it could not settle rather than
//! guessing at them — because a made-up year is indistinguishable downstream
//! from a real one, and an absence is not.
//!
//! What it will not do is resolve `ibid.`. A continuation refers to whatever
//! was cited before it, which is a fact about the surrounding page and not
//! about the entry; the entry is flagged as a continuation and left alone.

pub mod parse;
pub mod prompt;
pub mod split;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What sort of work a reference points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A whole book, with a publisher.
    Book,
    /// An article in a journal.
    Article,
    /// A piece within a larger edited work.
    Chapter,
    /// Something whose address is a URL.
    Web,
    /// Recognisably a reference, not recognisably any of the above.
    Other,
}

impl Kind {
    /// What it is called.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Book => "book",
            Kind::Article => "article",
            Kind::Chapter => "chapter",
            Kind::Web => "web",
            Kind::Other => "other",
        }
    }
}

/// One reference, as found and as far as it could be read.
///
/// Every field is written on every record, `null` where there is nothing.
/// A key that comes and goes is a key every consumer has to guard, and the
/// whole point of `unsettled` is that an absence be legible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Reference {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// Position in the block, from zero.
    pub idx: u32,
    /// The entry exactly as it appeared, with its own whitespace collapsed.
    ///
    /// Kept whatever else was read, because every other field on this record
    /// is an interpretation of this one and somebody will want to check it.
    pub text: String,
    /// The note number this entry carried, where it carried one.
    pub marker: Option<String>,
    /// Who wrote it, in the order the entry gave them.
    pub authors: Vec<String>,
    /// The year of publication.
    pub year: Option<u32>,
    /// The title of the work cited — the article, not the journal it is in.
    pub title: Option<String>,
    /// What it appeared in: a journal, or the publisher of a book.
    pub container: Option<String>,
    /// The pages, as written.
    pub pages: Option<String>,
    /// Any identifiers the entry carried, normalised.
    pub ids: Vec<Id>,
    /// What sort of work it is.
    pub kind: Kind,
    /// Whether this entry only points back at an earlier one: `ibid.`,
    /// `op. cit.`, and the rest. What it points at is not read off the entry,
    /// because the entry does not say.
    pub continuation: bool,
    /// The fields this entry did not settle, named so a caller can ask
    /// somebody else about exactly those and no more.
    pub unsettled: Vec<String>,
}

/// An identifier found in a reference, normalised to the form it stores in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Id {
    /// Which kind it is: `doi`, `isbn`, `issn`, `arxiv`, or `url`.
    pub kind: String,
    /// The identifier itself.
    pub id: String,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Reference {
    fn version(&self) -> u32 {
        self.v
    }
}

/// The fields a complete reference has, in the order they are reported.
pub const FIELDS: [&str; 4] = ["authors", "year", "title", "container"];

impl Reference {
    /// An empty reference carrying only the text it was cut from.
    pub fn of(text: &str) -> Self {
        Reference {
            v: corpus_core::V,
            idx: 0,
            text: text.to_string(),
            marker: None,
            authors: Vec::new(),
            year: None,
            title: None,
            container: None,
            pages: None,
            ids: Vec::new(),
            kind: Kind::Other,
            continuation: false,
            unsettled: Vec::new(),
        }
    }

    /// Which of the four fields worth having are still empty.
    ///
    /// A continuation is excluded: it has nothing to settle, and asking about
    /// one invites the guess this refuses to make.
    pub fn missing(&self) -> Vec<&'static str> {
        if self.continuation {
            return Vec::new();
        }
        FIELDS
            .into_iter()
            .filter(|field| match *field {
                "authors" => self.authors.is_empty(),
                "year" => self.year.is_none(),
                "title" => self.title.is_none(),
                _ => self.container.is_none(),
            })
            .collect()
    }
}

/// Read every reference in a block of text.
///
/// The block is cut into entries and each is read on its own. Entries that
/// carry nothing citation-shaped at all are dropped: a bibliography page has
/// a heading on it, and a heading is not a reference.
pub fn read(text: &str) -> Vec<Reference> {
    let mut out: Vec<Reference> = split::entries(text)
        .into_iter()
        .filter_map(|entry| parse::one(&entry.text, entry.marker.as_deref()))
        .collect();

    let mut previous: Vec<String> = Vec::new();
    for (at, reference) in out.iter_mut().enumerate() {
        reference.idx = at as u32;
        // A rule of dashes where the author should be means the author of
        // the entry above. That is not a guess: the style defines it, and
        // the entry it refers to is the one immediately before. `ibid.` is
        // refused because it points at a work, which may be pages back.
        if reference.authors.is_empty() && parse::repeats_author(&reference.text) {
            reference.authors = previous.clone();
        }
        if !reference.authors.is_empty() {
            previous = reference.authors.clone();
        }
        reference.unsettled = reference.missing().iter().map(|s| s.to_string()).collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_reports_what_it_does_not_have() {
        let mut one = Reference::of("something");
        assert_eq!(one.missing(), FIELDS);
        one.year = Some(1998);
        one.title = Some("A Title".into());
        assert_eq!(one.missing(), ["authors", "container"]);
    }

    #[test]
    fn a_continuation_has_nothing_to_ask_about() {
        let mut one = Reference::of("Ibid., 183.");
        one.continuation = true;
        assert!(one.missing().is_empty());
    }

    #[test]
    fn a_block_of_nothing_yields_nothing() {
        assert!(read("").is_empty());
        assert!(read("Bibliography\n\n").is_empty());
    }
}
