//! Section boundaries must match the Python exactly.
//!
//! A section becomes its own work in a catalogue, with its own identifier
//! that citations are written against. Splitting a collection differently
//! renames every work in it.

use serde_json::Value;

#[test]
fn every_collection_splits_the_same_as_python() {
    let cases = parity::cases("mdsplit");
    assert!(!cases.is_empty(), "no cases found");

    let mut wrong: Vec<String> = Vec::new();
    for case in cases {
        let name = parity::case_name(&case);
        let markdown = std::fs::read_to_string(&case).expect("a readable case");
        let theirs: Value =
            serde_json::from_str(&parity::read_golden_text(&format!("mdsplit/{name}.json")))
                .expect("valid JSON");

        let mut differs = |what: &str, ours: String, theirs: &Value| {
            wrong.push(format!("{name} {what}: python {theirs}, rust {ours}"));
        };

        let headings = mdsplit::headings_at(&markdown, 1);
        let ours = mdsplit::bible::looks_like_scripture(&headings);
        if ours != theirs["looks_like_bible"].as_bool().expect("a verdict") {
            differs(
                "looks_like_bible",
                ours.to_string(),
                &theirs["looks_like_bible"],
            );
        }

        let ours = mdsplit::split(&markdown);
        if ours.is_ok() != theirs["split"].as_bool().expect("a verdict") {
            differs("split", ours.is_ok().to_string(), &theirs["split"]);
            continue;
        }

        let ours = ours.unwrap_or_default();
        let theirs = theirs["sections"].as_array().expect("a list");
        for (at, (ours, theirs)) in ours.iter().zip(theirs).enumerate() {
            if ours.idx as u64 != theirs["idx"].as_u64().expect("an index") {
                differs(
                    &format!("section {at} idx"),
                    ours.idx.to_string(),
                    &theirs["idx"],
                );
            }
            if Some(ours.title.as_str()) != theirs["title"].as_str() {
                differs(
                    &format!("section {at} title"),
                    ours.title.clone(),
                    &theirs["title"],
                );
            }
            if Some(ours.slug.as_str()) != theirs["slug"].as_str() {
                differs(
                    &format!("section {at} slug"),
                    ours.slug.clone(),
                    &theirs["slug"],
                );
            }
            if Some(ours.text.as_str()) != theirs["text"].as_str() {
                differs(
                    &format!("section {at} text"),
                    format!("{} characters", ours.text.chars().count()),
                    &Value::from(
                        theirs["text"]
                            .as_str()
                            .map(|text| text.chars().count())
                            .unwrap_or(0),
                    ),
                );
            }
        }
        if ours.len() != theirs.len() {
            wrong.push(format!(
                "{name}: python found {} sections, rust {}",
                theirs.len(),
                ours.len()
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "{} differences:\n{}",
        wrong.len(),
        wrong
            .iter()
            .take(20)
            .map(|line| line.chars().take(300).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_files_still_match_live_python() {
    for case in parity::cases("mdsplit") {
        let name = parity::case_name(&case);
        let markdown = std::fs::read_to_string(&case).expect("a readable case");
        let live = parity::run_oracle("mdsplit.py", &markdown).expect("the oracle runs");
        assert_eq!(
            live,
            parity::read_golden_text(&format!("mdsplit/{name}.json")),
            "golden file for {name} is stale"
        );
    }
}
