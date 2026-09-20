//! A book's own table of contents.
//!
//! A book usually knows its own chapters. An EPUB carries them in a
//! navigation document, a PDF in an outline, and both are put there by the
//! publisher — which makes them better than any heading a converter guesses
//! at afterwards, because they are what the book itself says its parts are.
//!
//! What comes back is the entries in reading order, each with how deeply it
//! is nested, so a flat stream still carries the shape of the book.

pub mod epub;
pub mod pdf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One entry of a table of contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Entry {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// Position in reading order, from zero.
    pub idx: u32,
    /// How deeply nested: nought for a part, one for a chapter under it.
    pub depth: u32,
    /// What the book calls it.
    pub title: String,
    /// Where it points: a document within the archive, or a page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// The page it begins on, where the source counts in pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Entry {
    fn version(&self) -> u32 {
        self.v
    }
}

/// Why a file has no table of contents to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// The format keeps one, and this file has none.
    None(&'static str),
    /// Not a format that keeps one.
    Unsupported,
    /// The file could not be opened or parsed.
    Unreadable(String),
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Missing::None(where_) => write!(f, "no {where_}"),
            Missing::Unsupported => write!(f, "not a format that carries a table of contents"),
            Missing::Unreadable(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Missing {}

/// Read a file's table of contents.
pub fn read(path: &std::path::Path) -> Result<Vec<Entry>, Missing> {
    let bytes = std::fs::read(path).map_err(|e| Missing::Unreadable(e.to_string()))?;
    from_bytes(&bytes)
}

/// The same, for bytes already in hand.
pub fn from_bytes(bytes: &[u8]) -> Result<Vec<Entry>, Missing> {
    if bytes.starts_with(b"PK\x03\x04") {
        return epub::read(bytes);
    }
    if bytes.starts_with(b"%PDF") {
        return pdf::read(bytes);
    }
    Err(Missing::Unsupported)
}

/// Number the entries in reading order.
///
/// Done once at the end rather than while walking, so that a reader of a
/// nested structure does not have to thread a counter through it.
pub fn number(entries: &mut [Entry]) {
    for (at, entry) in entries.iter_mut().enumerate() {
        entry.idx = at as u32;
    }
}

/// Collapse runs of whitespace, which navigation documents are full of.
pub fn tidy(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_format_with_nowhere_to_keep_one_says_so() {
        assert_eq!(from_bytes(b"just some text"), Err(Missing::Unsupported));
    }

    #[test]
    fn entries_are_numbered_in_reading_order() {
        let mut entries: Vec<Entry> = (0..3)
            .map(|_| Entry {
                v: corpus_core::V,
                idx: 99,
                depth: 0,
                title: "x".into(),
                target: None,
                page: None,
            })
            .collect();
        number(&mut entries);
        assert_eq!(entries.iter().map(|e| e.idx).collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn whitespace_in_a_title_is_collapsed() {
        assert_eq!(tidy("  Chapter\n  One  "), "Chapter One");
        assert_eq!(tidy(""), "");
    }
}
