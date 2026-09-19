//! Fused ranks and scores must match the retrieval layer exactly.
//!
//! A fused score is a sum of reciprocals, so the order it produces depends on
//! the arithmetic being done the same way and on ties being broken the same
//! way. Neither is visible in a result list — it just comes out in a slightly
//! different order than the system it is meant to replace.

use corpus_core::Hit;
use serde_json::Value;

fn hits(case: &std::path::Path) -> Vec<Hit> {
    let text = std::fs::read_to_string(case).expect("a readable case");
    corpus_core::ndjson::read(text.as_bytes())
        .collect::<Result<_, _>>()
        .expect("valid hit records")
}

#[test]
fn every_case_fuses_the_same_as_the_retrieval_layer() {
    let cases = parity::cases_with_extension("rrf", "ndjson");
    assert!(!cases.is_empty(), "no cases found");

    for case in cases {
        let name = parity::case_name(&case);
        let golden = parity::read_golden_text(&format!("rrf/{name}.ndjson"));
        let theirs: Vec<Value> = golden
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("valid JSON"))
            .collect();
        let ours = rrf::fuse(&hits(&case), &rrf::Weights::default());

        for (at, (ours, theirs)) in ours.iter().zip(&theirs).enumerate() {
            assert_eq!(
                ours.id,
                theirs["id"].as_str().expect("an id"),
                "{name}: position {at} holds a different hit"
            );
            assert_eq!(
                ours.rank as u64,
                theirs["rank"].as_u64().expect("a rank"),
                "{name}: {} ranks differently",
                ours.id
            );
            assert_eq!(
                ours.score,
                theirs["score"].as_f64().expect("a score"),
                "{name}: {} scores differently",
                ours.id
            );
        }
        assert_eq!(
            ours.len(),
            theirs.len(),
            "{name}: python fused {} hits, rust {}",
            theirs.len(),
            ours.len()
        );
    }
}

/// Catches a golden file drifting away from a retrieval layer that has moved
/// on — in particular a change to the rank constant or the metadata weight.
#[test]
#[cfg(feature = "python")]
fn the_golden_files_still_match_live_python() {
    for case in parity::cases_with_extension("rrf", "ndjson") {
        let name = parity::case_name(&case);
        let input = std::fs::read_to_string(&case).expect("a readable case");
        let live = parity::run_oracle("rrf.py", &input).expect("the oracle runs");
        assert_eq!(
            live,
            parity::read_golden_text(&format!("rrf/{name}.ndjson")),
            "golden file for {name} is stale"
        );
    }
}
