//! Cue streams must match the JavaScript the archive was indexed with.
//!
//! A cue that shifts by a tenth of a second is a search result that lands in
//! the wrong sentence; a cue that is dropped is a sentence that cannot be
//! found at all. Both are invisible until somebody searches for exactly that
//! line.

use serde_json::Value;

/// A cue, as the two sides can be compared.
///
/// Not as JSON values: JavaScript writes a whole number as `5` where Rust
/// writes `5.0`, and the two parse to different kinds of JSON number for the
/// same quantity.
type Cue = (f64, f64, String);

fn cues(text: &str) -> Vec<Cue> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let cue: Value = serde_json::from_str(line).expect("valid JSON");
            (
                cue["start"].as_f64().expect("a start time"),
                cue["end"].as_f64().expect("an end time"),
                cue["text"].as_str().expect("text").to_string(),
            )
        })
        .collect()
}

fn ours(content: &str) -> Vec<Cue> {
    cuekit::merge_repeats(&cuekit::parse(content))
        .iter()
        .map(|cue| (cue.start, cue.end, cue.text.clone()))
        .collect()
}

#[test]
fn every_case_parses_the_same_as_the_original() {
    let cases = parity::cases_with_extension("cuenorm", "srt");
    assert!(!cases.is_empty(), "no cases found");

    for case in cases {
        let name = parity::case_name(&case);
        if parity::DIVERGENT_CUENORM.contains(&name.as_str()) {
            continue;
        }
        let content = std::fs::read_to_string(&case).expect("a readable case");
        let theirs = cues(&parity::read_golden_text(&format!("cuenorm/{name}.ndjson")));
        let ours = ours(&content);

        for (at, (ours, theirs)) in ours.iter().zip(&theirs).enumerate() {
            assert_eq!(ours, theirs, "{name} cue {at} differs");
        }

        assert_eq!(
            ours.len(),
            theirs.len(),
            "{name}: the original found {} cues, this found {}",
            theirs.len(),
            ours.len()
        );
    }
}

/// The divergence is that a cue line carrying screen coordinates after the
/// end time still reads. The original drops such a cue whole.
#[test]
fn the_documented_divergence_is_exactly_what_was_argued_for() {
    for name in parity::DIVERGENT_CUENORM {
        let case = parity::dir()
            .join("cases/cuenorm")
            .join(format!("{name}.srt"));
        let content = std::fs::read_to_string(&case).expect("a readable case");
        let theirs = cues(&parity::read_golden_text(&format!("cuenorm/{name}.ndjson")));
        let ours = ours(&content);
        assert!(
            ours.len() > theirs.len(),
            "{name} no longer recovers more than the original — update KNOWN.md"
        );
        // Everything the original did find, this finds too, unchanged.
        for cue in &theirs {
            assert!(ours.contains(cue), "{name} lost a cue the original kept");
        }
    }
}

/// Catches a golden file drifting away from an original that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_files_still_match_the_live_original() {
    for case in parity::cases_with_extension("cuenorm", "srt") {
        let name = parity::case_name(&case);
        let content = std::fs::read_to_string(&case).expect("a readable case");
        let live = parity::run_node_oracle("cuenorm.mjs", &content).expect("the oracle runs");
        let golden = parity::read_golden_text(&format!("cuenorm/{name}.ndjson"));
        assert_eq!(live, golden, "golden file for {name} is stale");
    }
}
