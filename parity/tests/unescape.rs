//! Character references must decode exactly as Python decodes them.
//!
//! The table is two thousand entries of specification data and the rules
//! around it exist only because browsers had to accept what was already on
//! the web. Getting one of them wrong leaves markup in the text of a book,
//! which is how `&lt;sup>107` reached thousands of passages before anyone
//! noticed.

#[test]
fn every_reference_decodes_the_same_as_python() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/unescape/entities.txt"))
        .expect("a readable case file");
    let cases: Vec<&str> = cases.split('\n').collect();
    let golden = parity::read_golden_text("unescape/entities.jsonl");
    let golden: Vec<String> = golden
        .lines()
        .map(|line| serde_json::from_str(line).expect("the oracle writes JSON strings"))
        .collect();

    assert_eq!(
        cases.len(),
        golden.len(),
        "{} cases against {} golden answers — regenerate",
        cases.len(),
        golden.len()
    );

    let mut wrong = Vec::new();
    for (case, expected) in cases.iter().zip(&golden) {
        let got = mdscrub::unescape::unescape(case);
        if &got != expected {
            wrong.push(format!("{case:?}: python {expected:?}, rust {got:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} references differ:\n{}",
        wrong.len(),
        cases.len(),
        wrong
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_live_python() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/unescape/entities.txt"))
        .expect("a readable case file");
    let live = parity::run_oracle("unescape.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("unescape/entities.jsonl"));
}
