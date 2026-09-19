//! Slugs, and the judgement about whether one should be replaced.
//!
//! A slug is a public identifier: it reaches citations and research notes and
//! outlives whatever produced it. Generating a different one than the system
//! being replaced would rename every book in a corpus, and judging one
//! differently would rename the wrong ones.

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
fn every_slug_and_verdict_matches_python() {
    let cases = records("cases/slug/records.ndjson");
    let golden = records("golden/slug/records.ndjson");
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (at, (case, theirs)) in cases.iter().zip(&golden).enumerate() {
        let author = field(case, "author");
        let title = field(case, "title");
        let slug = field(case, "slug").unwrap_or("");
        let id = case["id"].as_i64().expect("an id");

        let mut differs = |what: &str, ours: String, theirs: &Value| {
            wrong.push(format!("record {at} {what}: python {theirs}, rust {ours}"));
        };

        let ours = bibclean::slug::slugify(title.unwrap_or(""), 0, false);
        if Some(ours.as_str()) != theirs["slugify"].as_str() {
            differs("slugify", ours, &theirs["slugify"]);
        }

        let ours = bibclean::slug::lead_author(author.unwrap_or(""));
        if Some(ours.as_str()) != theirs["lead_author"].as_str() {
            differs("lead_author", ours, &theirs["lead_author"]);
        }

        let ours = bibclean::slug::slug_for(author, title, id);
        if Some(ours.as_str()) != theirs["slug_for"].as_str() {
            differs("slug_for", ours, &theirs["slug_for"]);
        }

        let proposal = bibclean::slug::evaluate(slug, author, title, id);
        let ours_tier = proposal.as_ref().map(|found| found.tier.as_str());
        if ours_tier != theirs["tier"].as_str() {
            differs("tier", format!("{ours_tier:?}"), &theirs["tier"]);
        }

        let ours_proposed = proposal.as_ref().map(|found| found.proposed.as_str());
        if ours_proposed != theirs["proposed"].as_str() {
            differs(
                "proposed",
                format!("{ours_proposed:?}"),
                &theirs["proposed"],
            );
        }

        let ours_reasons: Vec<String> = proposal
            .as_ref()
            .map(|found| found.reasons.clone())
            .unwrap_or_default();
        let their_reasons: Vec<String> = theirs["reasons"]
            .as_array()
            .expect("a list")
            .iter()
            .map(|reason| reason.as_str().expect("a reason").to_string())
            .collect();
        if ours_reasons != their_reasons {
            differs("reasons", format!("{ours_reasons:?}"), &theirs["reasons"]);
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
    let cases = std::fs::read_to_string(parity::dir().join("cases/slug/records.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("slug.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("slug/records.ndjson"));
}
