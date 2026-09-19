//! An inverted abstract has to come back as the same prose the Python gave.
//!
//! What the catalogue publishes is an index rather than the paragraph, and
//! what a reader sees on a search result is whatever this reconstructs. A
//! word in the wrong place is a sentence that reads as though it means
//! something and does not.

use bibclean::abstracts::{self, Inverted};
use serde_json::Value;

fn records(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

/// The cases, as text, so the index keeps the order it was written in.
///
/// Reading through `Value` would sort the keys, and which of two words at one
/// position comes first is exactly what that order decides.
fn cases() -> Vec<Value> {
    let text = std::fs::read_to_string(parity::dir().join("cases/abstracts/cases.ndjson"))
        .expect("a readable case file");
    records(&text)
}

#[test]
fn every_abstract_reads_back_the_same_as_python() {
    let golden = records(&parity::read_golden_text("abstracts/cases.ndjson"));
    let cases = cases();
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (case, theirs) in cases.iter().zip(&golden) {
        let name = case["name"].as_str().expect("a name");
        assert_eq!(theirs["name"].as_str(), Some(name), "cases out of order");

        let index: Inverted =
            serde_json::from_str(&case["index"].to_string()).expect("an index of positions");
        let ours = abstracts::from_inverted(&index);
        if ours.as_deref() != theirs["abstract"].as_str() {
            wrong.push(format!(
                "{name}: python {}, rust {ours:?}",
                theirs["abstract"]
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "{} abstracts differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_live_python() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/abstracts/cases.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("abstracts.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("abstracts/cases.ndjson"));
}
