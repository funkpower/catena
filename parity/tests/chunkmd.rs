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

/// Whether the only difference is a footnote flag the Rust sets and the
/// Python does not.
///
/// `looks_like_citations` learnt to recognise a run of reference entries
/// with nothing marking them — no numbers, no heading — which is how a
/// reader or a companion sets its end-of-chapter reading list. The Python it
/// was ported from never did, so it left those chunks unflagged and the
/// citation stage never selected them. Recorded in KNOWN.md; additive only,
/// so a flag the Python sets and the Rust does not is still a failure.
fn only_gains_the_footnote_flag(ours: &Value, theirs: &Value) -> bool {
    let flags = |v: &Value| -> Vec<String> {
        v.get("flags")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|f| f.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let (mine, yours) = (flags(ours), flags(theirs));
    if !yours.iter().all(|f| mine.contains(f)) {
        return false;
    }
    let gained: Vec<&String> = mine.iter().filter(|f| !yours.contains(f)).collect();
    if gained != [&"footnote_citations".to_string()] {
        return false;
    }
    // Everything else about the chunk has to be identical.
    ["idx", "page", "chapter", "role", "tokens", "text"]
        .iter()
        .all(|field| ours.get(field) == theirs.get(field))
}

fn compare(case: &str, set: &str, ours: &[Value], theirs: &[Value]) {
    for (at, (ours, theirs)) in ours.iter().zip(theirs).enumerate() {
        if ours != theirs && !only_gains_the_footnote_flag(ours, theirs) {
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
