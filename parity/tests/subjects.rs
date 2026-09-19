//! Every judgement a subject scheme makes must match the Python exactly.
//!
//! A scheme decides what a shelf can be browsed by. A heading mapped onto the
//! wrong subject files a book where nobody will look for it, and a fiction
//! genre left on a biography puts a life of somebody in among the novels.

use bibclean::scheme::Scheme;
use serde_json::Value;

fn records(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

fn cases() -> Vec<Value> {
    let text = std::fs::read_to_string(parity::dir().join("cases/subjects/cases.ndjson"))
        .expect("a readable case file");
    records(&text)
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|item| item.as_str().expect("a string").to_string())
        .collect()
}

#[test]
fn every_judgement_matches_python() {
    let golden = records(&parity::read_golden_text("subjects/cases.ndjson"));
    let cases = cases();
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (case, theirs) in cases.iter().zip(&golden) {
        let name = case["name"].as_str().expect("a name");
        let kind = case["kind"].as_str().expect("a kind");
        assert_eq!(theirs["name"].as_str(), Some(name), "cases out of order");

        // The scheme travels with the case, because a scheme is exactly the
        // thing that is not compiled in.
        let scheme: Scheme =
            serde_json::from_value(case["scheme"].clone()).expect("a readable scheme");
        let title = case["title"].as_str();

        let mut note = |field: &str, ours: String, theirs: &Value| {
            wrong.push(format!("{name}.{field}: python {theirs}, rust {ours}"));
        };

        match kind {
            "canonical" => {
                let ours = scheme.canonical(case["tag"].as_str().expect("a tag"));
                if ours != theirs["subject"].as_str() {
                    note("subject", format!("{ours:?}"), &theirs["subject"]);
                }
            }
            "parents" => {
                let ours = scheme.with_parents(&strings(&case["subjects"]));
                if ours != strings(&theirs["subjects"]) {
                    note("subjects", format!("{ours:?}"), &theirs["subjects"]);
                }
            }
            "fiction" => {
                let ours = scheme.without_impossible_fiction(&strings(&case["subjects"]), title);
                if ours != strings(&theirs["subjects"]) {
                    note("subjects", format!("{ours:?}"), &theirs["subjects"]);
                }
            }
            "compose" => {
                let ours = scheme.compose(&strings(&case["subjects"]), title);
                if ours != strings(&theirs["subjects"]) {
                    note("subjects", format!("{ours:?}"), &theirs["subjects"]);
                }
            }
            "needs" => {
                let ours = scheme.needs_subject(&strings(&case["existing"]));
                if Some(ours) != theirs["needs"].as_bool() {
                    note("needs", ours.to_string(), &theirs["needs"]);
                }
            }
            other => panic!("nothing answers a {other} case"),
        }
    }

    assert!(
        wrong.is_empty(),
        "{} judgements differ:\n{}",
        wrong.len(),
        wrong
            .iter()
            .take(20)
            .map(|line| line.chars().take(200).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Every question has to be asked, or the file proves less than it looks like
/// it does.
#[test]
fn every_kind_of_case_is_present() {
    let cases = cases();
    for kind in ["canonical", "parents", "fiction", "compose", "needs"] {
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
    let cases = std::fs::read_to_string(parity::dir().join("cases/subjects/cases.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("subjects.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("subjects/cases.ndjson"));
}
