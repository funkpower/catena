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

pub mod ask;
pub mod source;

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unreachable(why) => write!(f, "{why}"),
            Error::Shape(why) => write!(f, "the answer could not be read: {why}"),
            Error::NoRegister(kind) => {
                write!(f, "no register here answers about {kind} identifiers")
            }
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
