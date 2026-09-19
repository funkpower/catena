//! The series a book belongs to, and which volume of it this is.
//!
//! A shelf groups by series and volume, and most catalogues have neither set
//! on most books. The consequence is visible rather than theoretical: four
//! records all reading "A History of Philosophy" with nothing to say which
//! volume any of them is, and five volumes of one work sorting as five
//! unrelated books.
//!
//! Two shapes are recoverable from text the catalogue already holds, and they
//! are not the same kind of fact.
//!
//! A **volume marker** in the title — "The Philokalia Vol 3", "Collected
//! Works, Volume 9", "(Raven Son Book 1)" — is a *position*, and gives both a
//! series and a number.
//!
//! **Publisher shorthand** — a title of "Aquinas (Companion)" against an
//! author of "Cambridge" — is only *membership*. Cambridge Companions are not
//! numbered, so filing one under a numbered series makes every one of them
//! display as volume one, asserting an order the data does not have. Those
//! get a series and no number.

pub mod publisher;
pub mod volume;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use publisher::publisher_series;
pub use volume::{parse_volume, roman_or_int};

/// What a title turned out to say about its series.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Proposal {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// The title it was read from.
    pub title: String,
    /// The author it was read against, where one was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// The series the book belongs to.
    pub series: String,
    /// Which volume, where the title said. Absent for a series that is
    /// membership rather than position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
    /// A title reconstructed from the shorthand, where one reliably can be.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_title: Option<String>,
    /// Which of the two shapes this was.
    pub method: Method,
}

/// How a series was arrived at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// A volume marker in the title: a position within a series.
    Volume,
    /// Publisher shorthand in a parenthetical: membership, not position.
    Publisher,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Proposal {
    fn version(&self) -> u32 {
        self.v
    }
}

/// What a title and author say about the series, if anything.
///
/// The volume marker is looked for first: it carries a number, and a number
/// is worth more than membership.
pub fn series_of(title: &str, author: Option<&str>) -> Option<Proposal> {
    if let Some((series, volume)) = parse_volume(title) {
        return Some(Proposal {
            v: corpus_core::V,
            title: title.to_string(),
            author: author.map(str::to_string),
            series,
            volume: Some(volume),
            proposed_title: None,
            method: Method::Volume,
        });
    }

    let (proposed_title, series) = publisher_series(title, author?)?;
    Some(Proposal {
        v: corpus_core::V,
        title: title.to_string(),
        author: author.map(str::to_string),
        series,
        volume: None,
        proposed_title,
        method: Method::Publisher,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_volume_marker_gives_a_series_and_a_number() {
        let found = series_of("The Philokalia Vol 3", None).expect("a series");
        assert_eq!(found.series, "The Philokalia");
        assert_eq!(found.volume, Some(3));
        assert_eq!(found.method, Method::Volume);
    }

    #[test]
    fn publisher_shorthand_gives_membership_and_no_number() {
        let found = series_of("Aquinas (Companion)", Some("Cambridge")).expect("a series");
        assert_eq!(found.series, "Cambridge Companions");
        assert_eq!(found.volume, None, "membership is not a position");
        assert_eq!(
            found.proposed_title.as_deref(),
            Some("The Cambridge Companion to Aquinas")
        );
    }

    #[test]
    fn a_volume_marker_wins_over_a_parenthetical() {
        // Both could match; the one carrying a number is worth more.
        let found = series_of("Aquinas (Companion) Vol 2", Some("Cambridge")).expect("a series");
        assert_eq!(found.method, Method::Volume);
        assert_eq!(found.volume, Some(2));
    }

    #[test]
    fn an_ordinary_title_proposes_nothing() {
        assert_eq!(series_of("Being and Time", Some("Heidegger")), None);
        assert_eq!(series_of("", None), None);
    }

    #[test]
    fn publisher_shorthand_needs_the_publisher() {
        // The same title against anybody else is just a title.
        assert_eq!(series_of("Aquinas (Companion)", Some("Kerr")), None);
        assert_eq!(series_of("Aquinas (Companion)", None), None);
    }
}
