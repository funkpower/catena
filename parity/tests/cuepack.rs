//! Passage boundaries, times and token counts must match the Python exactly.
//!
//! A passage that starts one cue later is a different embedding of different
//! words at a different timestamp — which is to say a different row, and a
//! re-ingest that replaces an index rather than extending it.

use cuekit::Cue;
use serde_json::Value;

/// A passage, as the two sides can be compared.
type Passage = (u64, f64, f64, u64, String);

fn passages(text: &str) -> Vec<Passage> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let passage: Value = serde_json::from_str(line).expect("valid JSON");
            (
                passage["idx"].as_u64().expect("an index"),
                passage["start_time"].as_f64().expect("a start"),
                passage["end_time"].as_f64().expect("an end"),
                passage["tokens"].as_u64().expect("a token count"),
                passage["text"].as_str().expect("text").to_string(),
            )
        })
        .collect()
}

fn cues_of(case: &std::path::Path) -> Vec<Cue> {
    let text = std::fs::read_to_string(case).expect("a readable case");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let cue: Value = serde_json::from_str(line).expect("valid JSON");
            Cue::new(
                cue["start"].as_f64().expect("a start"),
                cue["end"].as_f64().expect("an end"),
                cue["text"].as_str().expect("text"),
            )
        })
        .collect()
}

/// The budget sets the packer is pinned at: the defaults, and a small set
/// that makes both flush conditions fire on a fixture short enough to read.
const BUDGETS: &[(&str, cuepack::Budgets)] = &[
    (
        "default",
        cuepack::Budgets {
            seconds: 75.0,
            max_tokens: 512,
        },
    ),
    (
        "small",
        cuepack::Budgets {
            seconds: 12.0,
            max_tokens: 24,
        },
    ),
];

#[test]
fn every_case_packs_the_same_as_python() {
    let cases = parity::cases_with_extension("cuepack", "ndjson");
    assert!(!cases.is_empty(), "no cases found");

    for case in cases {
        let name = parity::case_name(&case);
        let cues = cues_of(&case);
        for (set, budgets) in BUDGETS {
            let theirs = passages(&parity::read_golden_text(&format!(
                "cuepack/{name}.{set}.ndjson"
            )));
            let ours: Vec<Passage> = cuepack::pack(&cues, budgets)
                .into_iter()
                .map(|passage| {
                    (
                        passage.idx as u64,
                        passage.start_time.expect("a start"),
                        passage.end_time.expect("an end"),
                        passage.tokens as u64,
                        passage.text,
                    )
                })
                .collect();

            for (at, (ours, theirs)) in ours.iter().zip(&theirs).enumerate() {
                assert_eq!(ours, theirs, "{name} [{set}] passage {at} differs");
            }
            assert_eq!(
                ours.len(),
                theirs.len(),
                "{name} [{set}]: python packed {} passages, rust {}",
                theirs.len(),
                ours.len()
            );
        }
    }
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_files_still_match_live_python() {
    for case in parity::cases_with_extension("cuepack", "ndjson") {
        let name = parity::case_name(&case);
        let cues = std::fs::read_to_string(&case).expect("a readable case");
        for (set, budgets) in BUDGETS {
            let live = parity::run_oracle_with(
                "cuepack.py",
                &[budgets.seconds.to_string(), budgets.max_tokens.to_string()],
                &cues,
            )
            .expect("the oracle runs");
            let golden = parity::read_golden_text(&format!("cuepack/{name}.{set}.ndjson"));
            assert_eq!(live, golden, "golden file for {name} [{set}] is stale");
        }
    }
}
