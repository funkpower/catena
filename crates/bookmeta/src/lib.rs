//! The metadata a file already carries.
//!
//! Every ebook format has somewhere to put the title, and almost every file
//! in a library has something written there — put by the publisher, by the
//! converter, or by whoever last touched it. Reading it costs nothing and
//! needs no network, and it is the first thing a catalogue wants.
//!
//! What comes back is what the file says, not what is true. A PDF whose title
//! is `Microsoft Word - chapter3.doc` is reported with that title and a note
//! saying where it came from; deciding it is rubbish is
//! [`bibclean`](https://git.pumpin.online/funkpower/catena)'s job, and
//! looking up what is actually true is [`bibfetch`]'s.
//!
//! [`bibfetch`]: https://git.pumpin.online/funkpower/catena

pub mod epub;
pub mod mobi;
pub mod pdf;
pub mod prompt;

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a file's metadata was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// An EPUB's package document, which is where the publisher put it.
    Opf,
    /// A PDF's information dictionary.
    PdfInfo,
    /// A Kindle book's MOBI header and the EXTH records after it.
    Mobi,
    /// Nothing was found to read.
    None,
}

impl Source {
    /// What it is called.
    pub fn name(self) -> &'static str {
        match self {
            Source::Opf => "opf",
            Source::PdfInfo => "pdf_info",
            Source::Mobi => "mobi",
            Source::None => "none",
        }
    }
}

/// What a file says about itself.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Metadata {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// The file it was read from.
    pub path: String,
    /// Where in the file it came from.
    pub source: String,
    /// The title, as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The people credited, in the order the file lists them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// Who published it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// The publisher's description of the book, as written: often HTML.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The publication date, in whatever form the file wrote it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// The language, as the file declares it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The series it belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    /// Which volume of the series.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_index: Option<String>,
    /// Identifiers the file carries, by scheme: `isbn`, `doi`, `uuid` and
    /// whatever else the publisher wrote.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub identifiers: BTreeMap<String, String>,
    /// Subject headings, as written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
    /// What was worth saying about the reading.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// The cover image the file carries.
    ///
    /// Not part of the record: a record is a line of text and this is an
    /// image. A library caller takes it from here.
    #[serde(skip)]
    #[schemars(skip)]
    pub cover: Option<Cover>,
}

/// A cover image, as the file stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    /// What kind of image it is, read off its first bytes: `image/jpeg`,
    /// `image/png`, `image/gif` or `image/bmp`.
    pub media_type: String,
    /// The image itself.
    pub bytes: Vec<u8>,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Metadata {
    fn version(&self) -> u32 {
        self.v
    }
}

impl Metadata {
    /// An empty record for a file nothing could be read from.
    pub fn empty(path: &str, source: Source) -> Self {
        Metadata {
            v: corpus_core::V,
            path: path.to_string(),
            source: source.name().to_string(),
            ..Metadata::default()
        }
    }

    /// Add a note.
    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// The fields a catalogue needs that this record has not got.
    ///
    /// What a model could be asked about, and the only thing worth asking it:
    /// everything else is already known.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.title.as_deref().unwrap_or("").trim().is_empty() {
            out.push("title");
        }
        if self.authors.is_empty() {
            out.push("authors");
        }
        if self.publisher.is_none() {
            out.push("publisher");
        }
        if self.date.is_none() {
            out.push("date");
        }
        if self.language.is_none() {
            out.push("language");
        }
        out
    }

    /// Whether anything at all was found.
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.authors.is_empty()
            && self.publisher.is_none()
            && self.description.is_none()
            && self.date.is_none()
            && self.language.is_none()
            && self.identifiers.is_empty()
            && self.subjects.is_empty()
    }
}

/// Read whatever a file carries, choosing by what the bytes say it is.
pub fn read(path: &std::path::Path) -> Metadata {
    let name = path.display().to_string();
    match std::fs::read(path) {
        Err(e) => {
            let mut meta = Metadata::empty(&name, Source::None);
            meta.note(format!("could not be read: {e}"));
            meta
        }
        Ok(bytes) => from_bytes(&name, &bytes),
    }
}

/// The same, for bytes already in hand.
pub fn from_bytes(path: &str, bytes: &[u8]) -> Metadata {
    if bytes.starts_with(b"PK\x03\x04") {
        return epub::read(path, bytes);
    }
    if bytes.starts_with(b"%PDF") {
        return pdf::read(path, bytes);
    }
    if mobi::is_mobi(bytes) {
        return mobi::read(path, bytes);
    }
    let mut meta = Metadata::empty(path, Source::None);
    meta.note("not a format this reads metadata out of");
    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_format_with_nowhere_to_put_metadata_says_so() {
        let meta = from_bytes("notes.txt", b"just some text");
        assert_eq!(meta.source, "none");
        assert!(meta.is_empty());
        assert_eq!(meta.notes, ["not a format this reads metadata out of"]);
    }

    #[test]
    fn what_is_missing_is_what_a_model_could_be_asked() {
        let mut meta = Metadata::empty("x.epub", Source::Opf);
        assert_eq!(
            meta.missing(),
            ["title", "authors", "publisher", "date", "language"]
        );
        meta.title = Some("The Philokalia".into());
        meta.authors = vec!["Palmer".into()];
        assert_eq!(meta.missing(), ["publisher", "date", "language"]);
    }

    #[test]
    fn a_title_of_whitespace_is_no_title() {
        let mut meta = Metadata::empty("x.epub", Source::Opf);
        meta.title = Some("   ".into());
        assert!(meta.missing().contains(&"title"));
    }

    #[test]
    fn a_file_that_cannot_be_opened_is_a_record_with_a_reason() {
        let meta = read(std::path::Path::new("/definitely/not/here.epub"));
        assert_eq!(meta.source, "none");
        assert!(meta.notes[0].starts_with("could not be read:"));
    }
}
