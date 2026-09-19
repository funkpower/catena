//! Author recovery must match the Python exactly.
//!
//! A recovered name is written back into a catalogue. Recovering a different
//! one — a publisher, a title fragment, a name with the initials in the wrong
//! place — puts a wrong fact where there was an honest gap, which is worse
//! than the gap.

use serde_json::Value;

fn records(path: &str) -> Vec<Value> {
    let text = std::fs::read_to_string(parity::dir().join(path)).expect("a readable file");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

fn field<'a>(record: &'a Value, name: &str) -> Option<&'a str> {
    record.get(name).and_then(Value::as_str)
}

/// A suggestion as JSON, so the two sides compare field for field.
fn as_value(found: &bibclean::author::Suggestion) -> Value {
    serde_json::json!({
        "author": found.author,
        "method": found.method,
        "confidence": found.confidence,
        "evidence": found.evidence,
        "isbn": found.isbn,
    })
}

fn list(found: &[bibclean::author::Suggestion]) -> Value {
    Value::Array(found.iter().map(as_value).collect())
}

#[test]
fn every_author_judgement_matches_python() {
    let cases = records("cases/author/records.ndjson");
    let golden = records("golden/author/records.ndjson");
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (at, (case, theirs)) in cases.iter().zip(&golden).enumerate() {
        let title = field(case, "title").unwrap_or("");
        let front = field(case, "front_matter");
        let candidate = field(case, "candidate");

        let mut differs = |what: &str, ours: String, theirs: &Value| {
            wrong.push(format!("record {at} {what}: python {theirs}, rust {ours}"));
        };

        let ours = list(&bibclean::author::from_front_matter(front.unwrap_or("")));
        if &ours != &theirs["front_matter"] {
            differs("front_matter", ours.to_string(), &theirs["front_matter"]);
        }

        let ours = list(&bibclean::author::from_title(title));
        if &ours != &theirs["title"] {
            differs("title", ours.to_string(), &theirs["title"]);
        }

        let ours = bibclean::author::best(title, front)
            .as_ref()
            .map_or(Value::Null, as_value);
        if &ours != &theirs["best"] {
            differs("best", ours.to_string(), &theirs["best"]);
        }

        if let Some(candidate) = candidate {
            let ours = bibclean::author::clean(candidate);
            if ours.as_deref() != theirs["clean"].as_str() {
                differs("clean", format!("{ours:?}"), &theirs["clean"]);
            }
            let ours = bibclean::author::name_prefix(candidate);
            if Some(ours.as_str()) != theirs["name_prefix"].as_str() {
                differs("name_prefix", ours, &theirs["name_prefix"]);
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "{} judgements differ:\n{}",
        wrong.len(),
        wrong
            .iter()
            .map(|line| line.chars().take(300).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_live_python() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/author/records.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("author.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("author/records.ndjson"));
}
