//! One line of a transcript, and when it was said.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A subtitle cue: a span of time and what was said in it.
///
/// Deliberately smaller than a chunk. A cue is what a subtitle file holds —
/// a few seconds, often half a sentence — and turning cues into passages
/// worth retrieving is a later stage's job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cue {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// Seconds from the start of the recording.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub start: f64,
    /// Seconds from the start of the recording.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub end: f64,
    /// What was said, with the markup taken out.
    pub text: String,
}

fn version() -> u32 {
    corpus_core::V
}

impl Cue {
    /// A cue.
    pub fn new(start: f64, end: f64, text: impl Into<String>) -> Self {
        Self {
            v: corpus_core::V,
            start,
            end,
            text: text.into(),
        }
    }

    /// How long the cue lasts. Negative if the file says it ends before it
    /// starts, which some do.
    pub fn seconds(&self) -> f64 {
        self.end - self.start
    }
}

impl corpus_core::Record for Cue {
    fn version(&self) -> u32 {
        self.v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_field_is_emitted() {
        let cue = Cue::new(12.5, 15.0, "the mill stood at the northern end");
        assert_eq!(
            serde_json::to_string(&cue).unwrap(),
            r#"{"v":1,"start":12.5,"end":15.0,"text":"the mill stood at the northern end"}"#
        );
    }

    #[test]
    fn a_cue_reads_back_identical() {
        let cue = Cue::new(0.0, 1.0 / 3.0, "x");
        let text = serde_json::to_string(&cue).unwrap();
        let back: Cue = serde_json::from_str(&text).unwrap();
        assert_eq!(back.end.to_bits(), cue.end.to_bits());
    }

    #[test]
    fn a_time_that_json_cannot_carry_is_refused() {
        let cue = Cue::new(f64::NAN, 1.0, "x");
        assert!(serde_json::to_string(&cue).is_err());
    }
}
