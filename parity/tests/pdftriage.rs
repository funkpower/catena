//! Every number a text-layer verdict is decided on must match the Python
//! exactly.
//!
//! The thresholds these feed were measured against three thousand real PDFs,
//! so a score that is a little different is a verdict that is occasionally
//! different — and a wrong "garbled" costs an hour of GPU time while a wrong
//! "healthy" puts unreadable text in the index for good. The samples are
//! synthetic: a parity test over a text-layer classifier needs the score
//! inputs, not anybody's book.

use serde_json::Value;

/// Both sides compute a count divided by a count, so agreement is exact or
/// the counts differ.
fn same(ours: f64, theirs: &Value) -> bool {
    match theirs {
        Value::Null => false,
        value => value.as_f64() == Some(ours),
    }
}

fn cases() -> Vec<Value> {
    let text = std::fs::read_to_string(parity::dir().join("cases/pdftriage/samples.ndjson"))
        .expect("a readable case file");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

#[test]
fn every_measurement_matches_python() {
    let golden: Vec<Value> = parity::read_golden_text("pdftriage/samples.ndjson")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect();
    let cases = cases();
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (case, theirs) in cases.iter().zip(&golden) {
        let name = case["name"].as_str().expect("a name");
        assert_eq!(theirs["name"].as_str(), Some(name), "cases out of order");
        let text = case["text"].as_str().expect("text");
        let body = pdftriage::score::body(text);

        let mut note = |field: &str, ours: String, theirs: &Value| {
            wrong.push(format!("{name}.{field}: python {theirs}, rust {ours}"));
        };

        if !same(body.len() as f64, &theirs["body"]) {
            note("body", body.len().to_string(), &theirs["body"]);
        }
        if !same(pdftriage::score::ascii_text_score(text), &theirs["ascii"]) {
            let ours = pdftriage::score::ascii_text_score(text);
            note("ascii", ours.to_string(), &theirs["ascii"]);
        }
        if !same(pdftriage::score::letter_share(&body), &theirs["letters"]) {
            let ours = pdftriage::score::letter_share(&body);
            note("letters", ours.to_string(), &theirs["letters"]);
        }
        if !same(pdftriage::score::latin1_share(&body), &theirs["latin1"]) {
            let ours = pdftriage::score::latin1_share(&body);
            note("latin1", ours.to_string(), &theirs["latin1"]);
        }

        let shape = pdftriage::score::word_shape(text, 40);
        match (shape, theirs["vowelled"].as_f64(), theirs["tidy"].as_f64()) {
            (None, None, None) => {}
            (Some((vowelled, tidy)), Some(their_vowelled), Some(their_tidy)) => {
                if vowelled != their_vowelled {
                    note("vowelled", vowelled.to_string(), &theirs["vowelled"]);
                }
                if tidy != their_tidy {
                    note("tidy", tidy.to_string(), &theirs["tidy"]);
                }
            }
            (ours, _, _) => note("word_shape", format!("{ours:?}"), &theirs["vowelled"]),
        }

        let english = pdftriage::english_prose_share(text, 80);
        match (english, theirs["english"].as_f64()) {
            (None, None) => {}
            (Some(ours), Some(theirs_value)) if ours == theirs_value => {}
            (ours, _) => note("english", format!("{ours:?}"), &theirs["english"]),
        }

        let decoded = pdftriage::reflect::undo(text);
        let their_decoded = theirs["decoded"].as_str();
        if decoded.as_deref() != their_decoded {
            note(
                "decoded",
                match &decoded {
                    None => "none".to_string(),
                    Some(text) => format!("{} characters", text.chars().count()),
                },
                &theirs["decoded"],
            );
        }
    }

    assert!(
        wrong.is_empty(),
        "{} measurements differ:\n{}",
        wrong.len(),
        wrong
            .iter()
            .take(20)
            .map(|line| line.chars().take(200).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_live_python() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/pdftriage/samples.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("pdftriage.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("pdftriage/samples.ndjson"));
}
