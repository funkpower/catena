//! Chunk boundaries, token counts, provenance, roles and flags must match the
//! Python exactly.
//!
//! A boundary that moves is not a cosmetic difference: every passage after it
//! is a different passage than the one already embedded, so a re-ingest would
//! silently replace an index rather than extend it.

use serde_json::Value;

fn records(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("the oracle writes valid JSON"))
        .collect()
}

fn rust_records(markdown: &str, budgets: &chunkmd::Budgets) -> Vec<Value> {
    chunkmd::chunk(markdown, budgets)
        .iter()
        .map(|chunk| serde_json::to_value(chunk).expect("a chunk serialises"))
        .collect()
}

fn compare(case: &str, set: &str, ours: &[Value], theirs: &[Value]) {
    for (at, (ours, theirs)) in ours.iter().zip(theirs).enumerate() {
        if ours != theirs {
            let fields = ["idx", "page", "chapter", "role", "tokens", "flags", "text"];
            let differing: Vec<String> = fields
                .iter()
                .filter(|field| ours.get(*field) != theirs.get(*field))
                .map(|field| {
                    format!(
                        "  {field}: python {}, rust {}",
                        theirs.get(field).unwrap_or(&Value::Null),
                        ours.get(field).unwrap_or(&Value::Null)
                    )
                })
                .collect();
            panic!(
                "{case} [{set}] chunk {at} differs:\n{}",
                differing.join("\n")
            );
        }
    }
    assert_eq!(
        ours.len(),
        theirs.len(),
        "{case} [{set}]: python produced {} chunks, rust {}",
        theirs.len(),
        ours.len()
    );
}

#[test]
fn every_case_chunks_the_same_as_python() {
    let cases = parity::cases("chunkmd");
    assert!(!cases.is_empty(), "no cases found");

    for case in cases {
        let name = parity::case_name(&case);
        let markdown = std::fs::read_to_string(&case).expect("a readable case");
        for (set, budgets) in parity::budgets::SETS {
            let golden = parity::read_golden_text(&format!("chunkmd/{name}.{set}.ndjson"));
            compare(
                &name,
                set,
                &rust_records(&markdown, budgets),
                &records(&golden),
            );
        }
    }
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_files_still_match_live_python() {
    for case in parity::cases("chunkmd") {
        let name = parity::case_name(&case);
        let markdown = std::fs::read_to_string(&case).expect("a readable case");
        for (set, budgets) in parity::budgets::SETS {
            let live = parity::run_oracle_with(
                "chunkmd.py",
                &parity::budgets::as_args(budgets),
                &markdown,
            )
            .expect("the oracle should run");
            let golden = parity::read_golden_text(&format!("chunkmd/{name}.{set}.ndjson"));
            assert_eq!(live, golden, "golden file for {name} [{set}] is stale");
        }
    }
}
