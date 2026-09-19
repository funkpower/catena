//! The verdict and the command line must match the media server's exactly.
//!
//! A wrong "it plays" ships a file that opens to a black screen on the device
//! it was converted for; a wrong "it does not" spends forty minutes of GPU
//! time re-encoding something that was already fine. The command line has to
//! agree argument for argument, because a dropped `-pix_fmt` or a `-movflags`
//! in the wrong place is a file that encodes successfully and still will not
//! play.

use serde_json::Value;

/// The encoder the server defaults to when nothing names one.
const DEFAULT_ENCODER: &str = "h264_nvenc";

/// Answers that are meant to differ, each argued for in `KNOWN.md`.
///
/// The original emits a destination beginning with a dash bare, and an
/// encoder reads it as an option rather than a file.
const DIVERGENT: &[(&str, &str)] = &[("a-relative-name-beginning-with-a-dash", "argv")];

fn named(name: &str) -> avplan::Encoder {
    *avplan::encoder::PREFERRED
        .iter()
        .find(|encoder| encoder.name() == name)
        .unwrap_or_else(|| panic!("{name} is not an encoder"))
}

fn records(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

fn cases() -> Vec<Value> {
    let text = std::fs::read_to_string(parity::dir().join("cases/avplan/cases.ndjson"))
        .expect("a readable case file");
    records(&text)
}

#[test]
fn every_verdict_matches_the_original() {
    let golden = records(&parity::read_golden_text("avplan/cases.ndjson"));
    let cases = cases();
    assert_eq!(cases.len(), golden.len(), "case count moved — regenerate");

    let mut wrong: Vec<String> = Vec::new();
    for (case, theirs) in cases.iter().zip(&golden) {
        let name = case["name"].as_str().expect("a name");
        let path = case["path"].as_str().expect("a path");
        assert_eq!(theirs["path"].as_str(), Some(path), "cases out of order");

        let streams = avplan::Streams {
            video: case["video"].as_str().map(str::to_string),
            audio: case["audio"].as_str().map(str::to_string),
            container: avplan::container_of(path),
        };
        let device = case["vaapi_device"]
            .as_str()
            .unwrap_or(avplan::DEFAULT_VAAPI_DEVICE);
        let output = case["output"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| avplan::output_beside(path));
        let chosen = named(case["encoder"].as_str().unwrap_or(DEFAULT_ENCODER));
        let ours = avplan::plan(path, &streams, Some(chosen), &output, device);

        let mut note = |field: &str, ours: String, theirs: &Value| {
            if DIVERGENT.contains(&(name, field)) {
                return;
            }
            wrong.push(format!("{name}.{field}: original {theirs}, rust {ours}"));
        };

        if Some(ours.playable) != theirs["playable"].as_bool() {
            note("playable", ours.playable.to_string(), &theirs["playable"]);
        }
        if Some(ours.copies_audio) != theirs["copies_audio"].as_bool() {
            note(
                "copies_audio",
                ours.copies_audio.to_string(),
                &theirs["copies_audio"],
            );
        }

        let their_encoder = theirs["encoder"].as_str();
        let our_encoder = ours.encoder.map(avplan::Encoder::name);
        if our_encoder != their_encoder {
            note("encoder", format!("{our_encoder:?}"), &theirs["encoder"]);
        }

        let their_argv: Vec<&str> = theirs["argv"]
            .as_array()
            .expect("an argument list")
            .iter()
            .map(|arg| arg.as_str().expect("a string argument"))
            .collect();
        if ours.argv != their_argv {
            note("argv", format!("{:?}", ours.argv), &theirs["argv"]);
        }
    }

    assert!(
        wrong.is_empty(),
        "{} verdicts differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// A divergence that has quietly gone away is a note in `KNOWN.md` that is no
/// longer true, which is worth knowing about.
#[test]
fn the_documented_divergence_is_exactly_what_was_argued_for() {
    let case = cases()
        .into_iter()
        .find(|case| case["name"] == "a-relative-name-beginning-with-a-dash")
        .expect("the case exists");
    let path = case["path"].as_str().expect("a path");
    let streams = avplan::Streams {
        video: case["video"].as_str().map(str::to_string),
        audio: case["audio"].as_str().map(str::to_string),
        container: avplan::container_of(path),
    };
    let ours = avplan::plan(
        path,
        &streams,
        Some(named("libx264")),
        &avplan::output_beside(path),
        avplan::DEFAULT_VAAPI_DEVICE,
    );
    assert_eq!(
        ours.argv.last().map(String::as_str),
        Some("./-clip.h264.mp4"),
        "the destination is no longer named so an encoder reads it as a file"
    );
}

/// Every branch of the encoder table has to be exercised, or the parity test
/// proves nothing about the three encoders nobody's laptop happens to have.
#[test]
fn every_encoder_appears_among_the_cases() {
    let named: Vec<String> = cases()
        .iter()
        .filter_map(|case| case["encoder"].as_str().map(str::to_string))
        .collect();
    for encoder in avplan::encoder::PREFERRED {
        assert!(
            named.iter().any(|name| name == encoder.name()),
            "no case uses {}",
            encoder.name()
        );
    }
}

/// Catches a golden file drifting away from an original that has moved on.
#[test]
#[cfg(feature = "python")]
fn the_golden_file_still_matches_the_live_original() {
    let cases = std::fs::read_to_string(parity::dir().join("cases/avplan/cases.ndjson"))
        .expect("a readable case file");
    let live = parity::run_node_oracle("compatible.mjs", &cases).expect("the oracle runs");
    assert_eq!(live, parity::read_golden_text("avplan/cases.ndjson"));
}
