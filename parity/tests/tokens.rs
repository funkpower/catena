//! Token counts must agree with the Python tokeniser exactly.
//!
//! A count that is one off moves a chunk boundary, and a chunk boundary that
//! moves means every passage after it in the document is a different passage
//! than the one already embedded in an index.

#[test]
fn every_line_counts_the_same_as_python() {
    let golden = parity::read_golden("tokens.counts");
    let sample = parity::sample::standard();
    let lines: Vec<&str> = sample.split('\n').collect();

    assert_eq!(
        golden.len(),
        lines.len(),
        "the golden file has {} counts for {} lines — regenerate it",
        golden.len(),
        lines.len()
    );

    let mut wrong = Vec::new();
    for (at, (line, expected)) in lines.iter().zip(&golden).enumerate() {
        let expected: usize = expected
            .parse()
            .unwrap_or_else(|e| panic!("golden line {}: {e}", at + 1));
        let got = textkit::count(line);
        if got != expected {
            wrong.push(format!(
                "line {}: python {expected}, rust {got}, text {line:?}",
                at + 1
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "{} of {} lines disagree:\n{}",
        wrong.len(),
        lines.len(),
        wrong
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Catches the golden file drifting away from a Python that has moved on.
/// Needs an interpreter with `tiktoken`, named by `CATENA_PARITY_PYTHON`.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_live_python() {
    let live = parity::run_oracle("tokens.py", &parity::sample::standard())
        .expect("the oracle should run");
    let live: Vec<&str> = live.lines().collect();
    let golden = parity::read_golden("tokens.counts");
    assert_eq!(live.len(), golden.len(), "line count moved");
    for (at, (live, golden)) in live.iter().zip(&golden).enumerate() {
        assert_eq!(live, golden, "golden file is stale at line {}", at + 1);
    }
}
