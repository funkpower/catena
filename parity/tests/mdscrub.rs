//! Cleaned markdown must match the Python byte for byte, except where the
//! Rust is meant to differ — and there, the difference must be exactly the
//! one that was argued for.

fn case_text(case: &std::path::Path) -> String {
    std::fs::read_to_string(case).expect("a readable case")
}

#[test]
fn every_case_cleans_the_same_as_python() {
    let cases = parity::cases("mdscrub");
    assert!(!cases.is_empty(), "no cases found");

    for case in cases {
        let name = parity::case_name(&case);
        let ours = mdscrub::scrub(&case_text(&case));
        let theirs = parity::read_golden_text(&format!("mdscrub/{name}.md"));
        assert_eq!(ours, theirs, "{name} differs from the python");
    }
}

/// The thing the break-tag rule exists to prevent.
#[test]
fn a_line_break_tag_never_welds_two_words_together() {
    let cleaned = mdscrub::scrub("the mill stood at the<br>northern end");
    assert!(!cleaned.contains("thenorthern"), "{cleaned:?}");
    assert!(cleaned.contains("the northern"), "{cleaned:?}");
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_files_still_match_live_python() {
    for case in parity::cases("mdscrub") {
        let name = parity::case_name(&case);
        let live = parity::run_oracle("mdscrub.py", &case_text(&case)).expect("the oracle runs");
        let golden = parity::read_golden_text(&format!("mdscrub/{name}.md"));
        assert_eq!(live, golden, "golden file for {name} is stale");
    }
}
