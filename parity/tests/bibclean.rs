//! Every judgement about a catalogue record must match the Python's.
//!
//! These decide what a cleanup run touches. A reason that fires here and not
//! there means a record is rewritten that should have been left alone, which
//! is the one failure mode a metadata tool cannot recover from.

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

#[test]
fn every_judgement_matches_python() {
    let cases = records("cases/bibclean/records.ndjson");
    let golden = records("golden/bibclean/records.ndjson");
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (at, (case, theirs)) in cases.iter().zip(&golden).enumerate() {
        let title = field(case, "title");
        let author = field(case, "author");
        let isbn = field(case, "isbn");

        let mut differs = |what: &str, ours: String, theirs: &Value| {
            wrong.push(format!("record {at} {what}: python {theirs}, rust {ours}"));
        };

        let reasons = bibclean::reasons::problems(title, author, isbn);
        let their_reasons: Vec<String> = theirs["reasons"]
            .as_array()
            .expect("a list")
            .iter()
            .map(|reason| reason.as_str().expect("a name").to_string())
            .collect();
        if reasons != their_reasons {
            differs("reasons", format!("{reasons:?}"), &theirs["reasons"]);
        }

        let severity = bibclean::reasons::severity(&reasons);
        if severity as u64 != theirs["severity"].as_u64().expect("a number") {
            differs("severity", severity.to_string(), &theirs["severity"]);
        }

        let ours = bibclean::reasons::title_in_author(title, author);
        if ours != theirs["title_in_author"].as_bool().expect("a verdict") {
            differs(
                "title_in_author",
                ours.to_string(),
                &theirs["title_in_author"],
            );
        }

        let ours = bibclean::reasons::looks_like_person(author.unwrap_or(""));
        if ours != theirs["looks_like_person"].as_bool().expect("a verdict") {
            differs(
                "looks_like_person",
                ours.to_string(),
                &theirs["looks_like_person"],
            );
        }

        let ours = bibclean::fix::is_shouting(title.unwrap_or(""));
        if ours != theirs["is_shouting"].as_bool().expect("a verdict") {
            differs("is_shouting", ours.to_string(), &theirs["is_shouting"]);
        }

        if let Some(title) = title {
            let ours = bibclean::fix::title_case(title);
            if Some(ours.as_str()) != theirs["title_case"].as_str() {
                differs("title_case", ours, &theirs["title_case"]);
            }
            let ours = bibclean::fix::clean_underscores(title);
            if Some(ours.as_str()) != theirs["clean_underscores"].as_str() {
                differs("clean_underscores", ours, &theirs["clean_underscores"]);
            }
        }

        let ours = bibclean::isbn::valid(isbn.unwrap_or(""));
        if ours != theirs["valid_isbn"].as_bool().expect("a verdict") {
            differs("valid_isbn", ours.to_string(), &theirs["valid_isbn"]);
        }

        let ours = field(case, "path").and_then(bibclean::isbn::from_filename);
        if ours.as_deref() != theirs["isbn_from_filename"].as_str() {
            differs(
                "isbn_from_filename",
                format!("{ours:?}"),
                &theirs["isbn_from_filename"],
            );
        }

        let ours = bibclean::reasons::normalise(author.unwrap_or(""));
        if Some(ours.as_str()) != theirs["normalise"].as_str() {
            differs("normalise", ours, &theirs["normalise"]);
        }
    }

    assert!(
        wrong.is_empty(),
        "{} judgements differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_live_python() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/bibclean/records.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("bibclean.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("bibclean/records.ndjson"));
}
