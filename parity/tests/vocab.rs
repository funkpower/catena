//! Vocabulary clustering must match the Python exactly.
//!
//! A merge proposal is a suggestion to collapse two catalogue headings into
//! one. Proposing a different set than the system being replaced means either
//! merging two real people or leaving a person split in half, and the first
//! is much harder to undo than it is to do.

use bibclean::vocab::{Item, Kind, Rules};
use serde_json::Value;

fn records(path: &str) -> Vec<Value> {
    let text = std::fs::read_to_string(parity::dir().join(path)).expect("a readable file");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

fn kind_of(case: &Value) -> Kind {
    match case["kind"].as_str().expect("a kind") {
        "author" => Kind::Author,
        "publisher" => Kind::Publisher,
        "tag" => Kind::Tag,
        other => panic!("unknown kind {other}"),
    }
}

fn items_of(case: &Value) -> Vec<Item> {
    case["items"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|item| Item {
            id: item["id"].as_i64().expect("an id"),
            name: item["name"].as_str().expect("a name").to_string(),
            count: item["count"].as_i64().expect("a count"),
        })
        .collect()
}

/// The device named after its owner is supplied rather than compiled in, so
/// the oracle's built-in list has to be handed over for the comparison.
fn rules() -> Rules {
    Rules {
        device_prefixes: vec!["alexskobo".into()],
    }
}

#[test]
fn every_cluster_matches_python() {
    let cases = records("cases/vocab/lists.ndjson");
    let golden = records("golden/vocab/lists.ndjson");
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (at, (case, theirs)) in cases.iter().zip(&golden).enumerate() {
        let kind = kind_of(case);
        let items = items_of(case);

        let mut differs = |what: &str, ours: String, theirs: &Value| {
            wrong.push(format!("list {at} {what}: python {theirs}, rust {ours}"));
        };

        // The keys every grouping rests on.
        for (item, theirs) in items.iter().zip(theirs["keys"].as_array().expect("a list")) {
            let ours = bibclean::vocab::normalise(&item.name);
            if Some(ours.as_str()) != theirs["norm"].as_str() {
                differs("norm", ours, &theirs["norm"]);
            }
            let ours = Value::from(bibclean::vocab::author_tokens(&item.name));
            if ours != theirs["author_tokens"] {
                differs("author_tokens", ours.to_string(), &theirs["author_tokens"]);
            }
            let ours = bibclean::vocab::publisher_key(&item.name);
            if Some(ours.as_str()) != theirs["pub_key"].as_str() {
                differs("pub_key", ours, &theirs["pub_key"]);
            }
            let ours = bibclean::vocab::tag_key(&item.name);
            if Some(ours.as_str()) != theirs["tag_key"].as_str() {
                differs("tag_key", ours, &theirs["tag_key"]);
            }
            let ours = bibclean::vocab::junk_reason(&item.name, &rules());
            if ours != theirs["junk"].as_str() {
                differs("junk", format!("{ours:?}"), &theirs["junk"]);
            }
        }

        if !items.is_empty() {
            let ours = bibclean::vocab::pick_canonical(kind, &items).name;
            if Some(ours.as_str()) != theirs["canonical"].as_str() {
                differs("canonical", ours, &theirs["canonical"]);
            }
        }

        let ours: Vec<Value> = bibclean::vocab::cluster(kind, &items, &rules())
            .iter()
            .map(|proposal| {
                serde_json::json!({
                    "action": proposal.action,
                    "keep": proposal.keep,
                    "drop": proposal.drop,
                    "keep_id": proposal.keep_id,
                    "drop_id": proposal.drop_id,
                    "keep_count": proposal.keep_count,
                    "drop_count": proposal.drop_count,
                    "method": proposal.method,
                    "confidence": proposal.confidence,
                    "evidence": proposal.evidence,
                })
            })
            .collect();
        let ours = Value::Array(ours);
        if ours != theirs["proposals"] {
            differs("proposals", ours.to_string(), &theirs["proposals"]);
        }
    }

    assert!(
        wrong.is_empty(),
        "{} judgements differ:\n{}",
        wrong.len(),
        wrong
            .iter()
            .take(10)
            .map(|line| line.chars().take(400).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_live_python() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/vocab/lists.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("vocab.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("vocab/lists.ndjson"));
}
