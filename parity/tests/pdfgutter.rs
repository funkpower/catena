//! The arithmetic that decides a two-up scan must match the Python exactly.
//!
//! The consequence of getting it wrong is not a wrong page count — it is a
//! book whose text comes out interleaved line by line between two pages,
//! which reads as prose and is not. Nobody notices that in a corpus until
//! they read a retrieved passage and find it is nonsense.

use serde_json::Value;

fn records(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

fn cases() -> Vec<Value> {
    let text = std::fs::read_to_string(parity::dir().join("cases/pdfgutter/cases.ndjson"))
        .expect("a readable case file");
    records(&text)
}

fn floats(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|number| number.as_f64().expect("a number"))
        .collect()
}

#[test]
fn every_answer_matches_python() {
    let golden = records(&parity::read_golden_text("pdfgutter/cases.ndjson"));
    let cases = cases();
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (case, theirs) in cases.iter().zip(&golden) {
        let name = case["name"].as_str().expect("a name");
        let kind = case["kind"].as_str().expect("a kind");
        assert_eq!(theirs["name"].as_str(), Some(name), "cases out of order");
        assert_eq!(theirs["kind"].as_str(), Some(kind), "cases out of order");

        let mut note = |field: &str, ours: String, theirs: &Value| {
            wrong.push(format!("{name}.{field}: python {theirs}, rust {ours}"));
        };

        match kind {
            "ratio" => {
                let ours = pdfgutter::aspect_ratio(
                    case["width"].as_f64().expect("a width"),
                    case["height"].as_f64().expect("a height"),
                );
                if Some(ours) != theirs["ratio"].as_f64() {
                    note("ratio", ours.to_string(), &theirs["ratio"]);
                }
            }
            "gutter" => {
                let ours = pdfgutter::has_gutter(&floats(&case["ink"]));
                if Some(ours) != theirs["gutter"].as_bool() {
                    note("gutter", ours.to_string(), &theirs["gutter"]);
                }
            }
            "sample" => {
                let ours = pdfgutter::sample_indices(
                    case["pages"].as_u64().expect("a page count") as usize
                );
                let theirs_indices: Vec<usize> = theirs["indices"]
                    .as_array()
                    .expect("an array")
                    .iter()
                    .map(|index| index.as_u64().expect("a number") as usize)
                    .collect();
                if ours != theirs_indices {
                    note("indices", format!("{ours:?}"), &theirs["indices"]);
                }
            }
            "greymap" => {
                let width = case["width"].as_u64().expect("a width") as usize;
                let height = case["height"].as_u64().expect("a height") as usize;
                let pixels: Vec<u8> = case["pixels"]
                    .as_array()
                    .expect("an array")
                    .iter()
                    .map(|value| value.as_u64().expect("a number") as u8)
                    .collect();

                // The renderer is not the thing under test; the reduction it
                // feeds is. Both sides are handed the same pixels.
                let mut greymap = format!("P5\n{width} {height}\n255\n").into_bytes();
                greymap.extend_from_slice(&pixels);
                let ours = pdfgutter::ink::profile(&greymap, pdfgutter::INK_THRESHOLD)
                    .expect("a readable greymap");

                let theirs_profile = floats(&theirs["profile"]);
                if ours != theirs_profile {
                    note("profile", format!("{ours:?}"), &theirs["profile"]);
                }
            }
            "verdict" => {
                let ratios = floats(&case["ratios"]);
                let gutters: Vec<bool> = case["gutters"]
                    .as_array()
                    .expect("an array")
                    .iter()
                    .map(|found| found.as_bool().expect("a boolean"))
                    .collect();

                let median = pdfgutter::median(&ratios);
                if Some(median) != theirs["median_ratio"].as_f64() {
                    note("median_ratio", median.to_string(), &theirs["median_ratio"]);
                }
                let fraction = pdfgutter::gutter_fraction(&gutters);
                if Some(fraction) != theirs["gutter_fraction"].as_f64() {
                    note(
                        "gutter_fraction",
                        fraction.to_string(),
                        &theirs["gutter_fraction"],
                    );
                }
                let two_up = pdfgutter::is_two_up(&ratios, &gutters);
                if Some(two_up) != theirs["two_up"].as_bool() {
                    note("two_up", two_up.to_string(), &theirs["two_up"]);
                }
            }
            other => panic!("nothing answers a {other} case"),
        }
    }

    assert!(
        wrong.is_empty(),
        "{} answers differ:\n{}",
        wrong.len(),
        wrong
            .iter()
            .take(20)
            .map(|line| line.chars().take(200).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// All four questions have to be asked, or the file proves less than it looks
/// like it does.
#[test]
fn every_kind_of_case_is_present() {
    let cases = cases();
    for kind in ["ratio", "gutter", "sample", "verdict", "greymap"] {
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
    let cases = std::fs::read_to_string(parity::dir().join("cases/pdfgutter/cases.ndjson"))
        .expect("a readable case file");
    let live = parity::run_oracle("pdfgutter.py", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("pdfgutter/cases.ndjson"));
}
