//! The identifiers a bibliography is held together by, read and checked.
//!
//! A DOI arrives as a bare string, a URL, a `doi:` prefix, with a trailing
//! full stop the sentence around it left behind, in whatever case the
//! publisher felt like. All of those are the same identifier, and a catalogue
//! that stores them as they arrived cannot tell that.
//!
//! Nothing here asks anybody whether an identifier exists. What it can say is
//! whether a string is *well formed*: an ISBN and an ISSN carry a check digit
//! that catches a mistyped one, and a DOI or an arXiv id has a shape. That is
//! a different question from whether the work is real, and the honest answer
//! to the second needs a catalogue.

pub mod arxiv;
pub mod doi;
pub mod issn;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What kind of identifier something is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A digital object identifier: `10.1000/182`.
    Doi,
    /// An international standard serial number, for a journal: `0028-0836`.
    Issn,
    /// An international standard book number, for a book.
    Isbn,
    /// An arXiv preprint identifier: `2401.01234` or `math.GT/0309136`.
    Arxiv,
}

impl Kind {
    /// What it is called.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Doi => "doi",
            Kind::Issn => "issn",
            Kind::Isbn => "isbn",
            Kind::Arxiv => "arxiv",
        }
    }
}

/// One identifier, as it arrived and as it should be stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Identifier {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// The text it was read from.
    pub input: String,
    /// Which kind it turned out to be.
    pub kind: Kind,
    /// The identifier, in the form it should be stored and compared in.
    pub id: String,
    /// Whether the check digit agrees, where the kind carries one. `None`
    /// where the kind has nothing to check against.
    pub checksum: Option<bool>,
    /// A URL that resolves it, where the kind has a resolver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Identifier {
    fn version(&self) -> u32 {
        self.v
    }
}

/// Read whatever kind of identifier a string holds.
///
/// Tried in order of how distinctive each shape is: a DOI announces itself
/// with `10.`, an arXiv id with its own prefix or a dotted archive name, and
/// what is left is a run of digits that is either thirteen or ten long (an
/// ISBN) or eight (an ISSN).
pub fn parse(text: &str) -> Option<Identifier> {
    parse_as(text, None)
}

/// Read a string as one particular kind, or as whatever it looks like.
pub fn parse_as(text: &str, kind: Option<Kind>) -> Option<Identifier> {
    let input = text.trim();
    if input.is_empty() {
        return None;
    }

    // Order matters: a DOI announces itself with `10.`, an arXiv id with its
    // own prefix, and what is left is a run of digits whose length decides
    // between a book and a journal.
    const EVERY: [Kind; 4] = [Kind::Doi, Kind::Arxiv, Kind::Isbn, Kind::Issn];
    let one = [kind.unwrap_or(Kind::Doi)];
    let kinds: &[Kind] = match kind {
        Some(_) => &one,
        None => &EVERY,
    };

    for kind in kinds {
        let found = match kind {
            Kind::Doi => doi::parse(input).map(|id| Identifier {
                v: corpus_core::V,
                url: Some(format!("https://doi.org/{id}")),
                input: input.to_string(),
                kind: Kind::Doi,
                id,
                checksum: None,
            }),
            Kind::Arxiv => arxiv::parse(input).map(|id| Identifier {
                v: corpus_core::V,
                url: Some(format!("https://arxiv.org/abs/{id}")),
                input: input.to_string(),
                kind: Kind::Arxiv,
                id,
                checksum: None,
            }),
            Kind::Issn => issn::parse(input).map(|id| Identifier {
                v: corpus_core::V,
                checksum: Some(issn::valid(&id)),
                url: Some(format!("https://portal.issn.org/resource/ISSN/{id}")),
                input: input.to_string(),
                kind: Kind::Issn,
                id,
            }),
            Kind::Isbn => isbn(input),
        };
        if let Some(found) = found {
            return Some(found);
        }
    }
    None
}

/// An ISBN, using the reader the catalogue tools already share.
fn isbn(input: &str) -> Option<Identifier> {
    let digits = bibclean::isbn::digits(input);
    if !matches!(digits.len(), 10 | 13) {
        return None;
    }
    // Only where the input is the identifier and nothing else. An ISBN has no
    // prefix that announces it, so a sentence holding a thirteen-digit number
    // would otherwise read as one.
    if !input
        .chars()
        .all(|c| c.is_ascii_digit() || "- xX".contains(c))
    {
        return None;
    }
    Some(Identifier {
        v: corpus_core::V,
        checksum: Some(bibclean::isbn::valid(&digits)),
        url: Some(format!("https://openlibrary.org/isbn/{digits}")),
        input: input.to_string(),
        kind: Kind::Isbn,
        id: digits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_doi_is_read_however_it_was_written() {
        for text in [
            "10.1000/182",
            "doi:10.1000/182",
            "https://doi.org/10.1000/182",
            "http://dx.doi.org/10.1000/182",
            " 10.1000/182. ",
        ] {
            let found = parse(text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(found.kind, Kind::Doi, "{text}");
            assert_eq!(found.id, "10.1000/182", "{text}");
        }
    }

    #[test]
    fn an_issn_carries_a_check_digit() {
        let found = parse("0028-0836").expect("an issn");
        assert_eq!(found.kind, Kind::Issn);
        assert_eq!(found.id, "0028-0836");
        assert_eq!(found.checksum, Some(true));

        let wrong = parse("0028-0837").expect("an issn");
        assert_eq!(wrong.checksum, Some(false), "one digit out is caught");
    }

    #[test]
    fn an_isbn_carries_one_too() {
        let found = parse("978-0-306-40615-7").expect("an isbn");
        assert_eq!(found.kind, Kind::Isbn);
        assert_eq!(found.id, "9780306406157");
        assert_eq!(found.checksum, Some(true));
    }

    #[test]
    fn an_arxiv_identifier_is_read_in_both_its_forms() {
        assert_eq!(parse("arXiv:2401.01234").expect("an id").id, "2401.01234");
        assert_eq!(parse("2401.01234v2").expect("an id").id, "2401.01234v2");
        assert_eq!(
            parse("math.GT/0309136").expect("an id").id,
            "math.GT/0309136"
        );
    }

    #[test]
    fn every_kind_resolves_somewhere() {
        for text in [
            "10.1000/182",
            "0028-0836",
            "978-0-306-40615-7",
            "arXiv:2401.01234",
        ] {
            assert!(parse(text).expect("an id").url.is_some(), "{text}");
        }
    }

    #[test]
    fn what_is_not_an_identifier_is_not_one() {
        for text in ["", "   ", "hello", "10.", "1234", "not/an/id"] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_kind_can_be_insisted_on() {
        // Eight digits are an ISSN; asked for a DOI, they are nothing.
        assert!(parse_as("00280836", Some(Kind::Issn)).is_some());
        assert_eq!(parse_as("00280836", Some(Kind::Doi)), None);
    }

    #[test]
    fn a_sentence_that_merely_contains_digits_is_not_an_isbn() {
        assert_eq!(parse("published in 9780306406157 by somebody"), None);
    }
}
