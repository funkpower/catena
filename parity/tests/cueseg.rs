//! Chapter boundaries, titles and excerpts must match the Python exactly.
//!
//! The boundaries come out of float arithmetic over hashed word counts, so a
//! difference of one word in the tokeniser or one unit in the last place in a
//! cosine moves a chapter mark by a minute. Nothing about that is visible in
//! the output; it just puts the mark in the wrong place.

use std::collections::BTreeMap;

use cuekit::Cue;
use serde_json::Value;

/// A chapter, as the two sides can be compared.
type Chapter = (f64, f64, String, String);

fn chapters(text: &str) -> Vec<Chapter> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let chapter: Value = serde_json::from_str(line).expect("valid JSON");
            (
                chapter["start"].as_f64().expect("a start"),
                chapter["end"].as_f64().expect("an end"),
                chapter["title"].as_str().expect("a title").to_string(),
                chapter["excerpt"].as_str().expect("an excerpt").to_string(),
            )
        })
        .collect()
}

fn ours(cues: &[Cue], duration: f64) -> Vec<Chapter> {
    cueseg::segment(cues, duration)
        .into_iter()
        .map(|chapter| (chapter.start, chapter.end, chapter.title, chapter.excerpt))
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

/// The two durations each case is run at: the end of the transcript, and a
/// recording that carries on after it.
fn durations() -> BTreeMap<String, BTreeMap<String, f64>> {
    let text = std::fs::read_to_string(parity::dir().join("cases/cueseg/durations.json"))
        .expect("a readable duration table");
    serde_json::from_str(&text).expect("valid JSON")
}

#[test]
fn every_transcript_segments_the_same_as_python() {
    let durations = durations();
    let cases = parity::cases_with_extension("cueseg", "ndjson");
    assert!(!cases.is_empty(), "no cases found");

    for case in cases {
        let name = parity::case_name(&case);
        let cues = cues_of(&case);
        let spans = durations.get(&name).expect("a duration for every case");

        for (span, duration) in spans {
            let theirs = chapters(&parity::read_golden_text(&format!(
                "cueseg/{name}.{span}.ndjson"
            )));
            let ours = ours(&cues, *duration);

            for (at, (ours, theirs)) in ours.iter().zip(&theirs).enumerate() {
                assert_eq!(
                    ours.0, theirs.0,
                    "{name} [{span}] chapter {at} starts elsewhere"
                );
                assert_eq!(
                    ours.1, theirs.1,
                    "{name} [{span}] chapter {at} ends elsewhere"
                );
                assert_eq!(
                    ours.2, theirs.2,
                    "{name} [{span}] chapter {at} is titled differently"
                );
                assert_eq!(
                    ours.3, theirs.3,
                    "{name} [{span}] chapter {at} excerpts differently"
                );
            }
            assert_eq!(
                ours.len(),
                theirs.len(),
                "{name} [{span}]: python found {} chapters, rust {}",
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
    let durations = durations();
    for case in parity::cases_with_extension("cueseg", "ndjson") {
        let name = parity::case_name(&case);
        let cues = std::fs::read_to_string(&case).expect("a readable case");
        for (span, duration) in durations.get(&name).expect("a duration") {
            let live = parity::run_oracle_with("cueseg.py", &[format!("{duration}")], &cues)
                .expect("the oracle runs");
            let golden = parity::read_golden_text(&format!("cueseg/{name}.{span}.ndjson"));
            assert_eq!(live, golden, "golden file for {name} [{span}] is stale");
        }
    }
}
