//! Every series and volume read off a title must match the Python exactly.
//!
//! The cost of getting it wrong is a shelf that looks right and is not: a
//! volume filed under the wrong series disappears from the work it belongs
//! to, and a title reconstructed from the wrong template renames a book to
//! something nobody published.

use serde_json::Value;

fn records(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

fn cases() -> Vec<Value> {
    let text = std::fs::read_to_string(parity::dir().join("cases/seriesof/cases.ndjson"))
        .expect("a readable case file");
    records(&text)
}

/// A string field, where `null` and absent mean the same thing.
fn text(value: &Value) -> Option<&str> {
    value.as_str()
}

#[test]
fn every_series_matches_python() {
    let golden = records(&parity::read_golden_text("seriesof/cases.ndjson"));
    let cases = cases();
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (case, theirs) in cases.iter().zip(&golden) {
        let name = case["name"].as_str().expect("a name");
        let kind = case["kind"].as_str().expect("a kind");
        assert_eq!(theirs["name"].as_str(), Some(name), "cases out of order");

        let mut note = |field: &str, ours: String, theirs: &Value| {
            wrong.push(format!("{name}.{field}: python {theirs}, rust {ours}"));
        };

        match kind {
            "number" => {
                let ours = seriesof::roman_or_int(case["text"].as_str().expect("text"));
                let theirs_number = theirs["number"].as_u64().map(|n| n as u32);
                if ours != theirs_number {
                    note("number", format!("{ours:?}"), &theirs["number"]);
                }
            }
            "volume" => {
                let found = seriesof::parse_volume(case["title"].as_str().expect("a title"));
                let (series, volume) = match &found {
                    Some((series, volume)) => (Some(series.as_str()), Some(*volume)),
                    None => (None, None),
                };
                if series != text(&theirs["series"]) {
                    note("series", format!("{series:?}"), &theirs["series"]);
                }
                let theirs_volume = theirs["volume"].as_u64().map(|n| n as u32);
                if volume != theirs_volume {
                    note("volume", format!("{volume:?}"), &theirs["volume"]);
                }
            }
            "publisher" => {
                let author = case["author"].as_str();
                let found = author.and_then(|author| {
                    seriesof::publisher_series(case["title"].as_str().expect("a title"), author)
                });
                let (proposed, series) = match &found {
                    Some((proposed, series)) => (proposed.as_deref(), Some(series.as_str())),
                    None => (None, None),
                };
                if proposed != text(&theirs["proposed_title"]) {
                    note(
                        "proposed_title",
                        format!("{proposed:?}"),
                        &theirs["proposed_title"],
                    );
                }
                if series != text(&theirs["series"]) {
                    note("series", format!("{series:?}"), &theirs["series"]);
                }
            }
            other => panic!("nothing answers a {other} case"),
        }
    }

    assert!(
        wrong.is_empty(),
        "{} answers differ:\n{}",
        wrong.len(),
        wrong
            .iter()
            .take(20)
            .map(|line| line.chars().take(200).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// All three questions have to be asked, or the file proves less than it
/// looks like it does.
#[test]
fn every_kind_of_case_is_present() {
    let cases = cases();
    for kind in ["number", "volume", "publisher"] {
        assert!(
            cases.iter().any(|case| case["kind"] == kind),
            "no {kind} case"
        );
    }
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_live_python() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/seriesof/cases.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("series.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("seriesof/cases.ndjson"));
}
