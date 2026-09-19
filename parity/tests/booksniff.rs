//! Container identification must agree with the Python's.
//!
//! The consequence of getting this wrong is not a wrong record: it is a
//! converter handed a file it cannot read, and a book that never enters the
//! corpus with nothing but an unhelpful error to say why.

use booksniff::bytes::Bytes;
use corpus_core::Kind;
use serde_json::Value;

fn cases() -> Vec<Value> {
    let text = std::fs::read_to_string(parity::dir().join("cases/booksniff/files.ndjson"))
        .expect("a readable case file");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

fn bytes_of(case: &Value) -> Vec<u8> {
    let hex = case["hex"].as_str().expect("hex bytes");
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("valid hex"))
        .collect()
}

#[test]
fn every_container_is_read_the_same_as_python() {
    let golden: Vec<Value> = parity::read_golden_text("booksniff/files.ndjson")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect();
    let cases = cases();
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    for (case, theirs) in cases.iter().zip(&golden) {
        let name = case["name"].as_str().expect("a name");
        assert_eq!(theirs["name"].as_str(), Some(name), "cases out of order");
        let bytes = bytes_of(case);
        let source = bytes.as_slice();
        let doc = booksniff::sniff(&source, name);

        // The three magic-byte tests, each of which the Python answers
        // directly.
        assert_eq!(
            doc.kind == Kind::Djvu,
            theirs["djvu"].as_bool().expect("a verdict"),
            "{name}: DjVu"
        );
        assert_eq!(
            doc.kind == Kind::Ole2,
            theirs["ole2"].as_bool().expect("a verdict"),
            "{name}: OLE2"
        );
        assert_eq!(
            source.magic_at(0, booksniff::zip::LOCAL_HEADER),
            theirs["zip"].as_bool().expect("a verdict"),
            "{name}: zip"
        );

        // What the archive holds, where the Python classifies one.
        if let Some(kind) = theirs["zip_kind"].as_str() {
            let ours = match doc.kind {
                Kind::Epub => "epub",
                Kind::ZipWrappedPdf => "pdf",
                _ if doc.notes.iter().any(|note| note.contains("HTML")) => "htmlz",
                _ => "other",
            };
            assert_eq!(ours, kind, "{name}: what the zip holds");
        }

        // And which member it would unwrap.
        if let Some(names) = booksniff::zip::members(&source) {
            assert_eq!(
                booksniff::sole_pdf(&names),
                theirs["sole_pdf"].as_str(),
                "{name}: the sole PDF"
            );
        }
    }
}

/// Catches a golden file drifting away from a Python that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_live_python() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/booksniff/files.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("booksniff.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("booksniff/files.ndjson"));
}
