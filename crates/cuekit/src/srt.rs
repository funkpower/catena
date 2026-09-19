//! SubRip, and the WebVTT that is close enough to share its reader.
//!
//! A port of the parser the podcast archive was indexed with, quirks kept:
//! the timestamp line is found by looking for an arrow anywhere in the block
//! rather than at a fixed position, only the first comma in a timestamp
//! becomes a point, and a block whose times do not read as numbers is
//! dropped whole rather than guessed at.

use std::sync::LazyLock;

use regex::Regex;

use crate::cue::Cue;
use crate::jsnum;
use crate::text;

/// Blocks are separated by one or more blank lines.
static BLOCKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\n+").expect("a valid pattern"));

/// Parse a SubRip file.
pub fn parse(content: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    for block in BLOCKS.split(content) {
        let lines: Vec<&str> = block.trim_matches(textkit::is_space).split('\n').collect();
        let Some(at) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let Some((start, end)) = times(lines[at]) else {
            continue;
        };
        let text = text::clean(&lines[at + 1..].join(" "));
        if !text.is_empty() {
            cues.push(Cue::new(start, end, text));
        }
    }
    cues
}

/// Read a timestamp line, or `None` if either end is not a number.
///
/// Each half is reduced to the one token that could be a timestamp, which is
/// what lets the same reader take a WebVTT cue line — where the end time is
/// followed by positioning settings — and an old SubRip file with screen
/// coordinates after it. See `parity/KNOWN.md`: the original reads each half
/// whole and drops such a cue entirely.
fn times(line: &str) -> Option<(f64, f64)> {
    let (before, after) = line.split_once("-->")?;
    let start = timestamp(before.split_whitespace().next_back().unwrap_or(""));
    let end = timestamp(after.split_whitespace().next().unwrap_or(""));
    (start.is_finite() && end.is_finite()).then_some((start, end))
}

/// Seconds from a `HH:MM:SS,mmm` or `MM:SS.mmm` timestamp.
///
/// Only the first comma is replaced, exactly as the original does — a
/// timestamp written with a comma for the thousands separator as well as for
/// the decimal point therefore parses as far as the second comma and no
/// further.
pub fn timestamp(text: &str) -> f64 {
    let text = text.trim_matches(textkit::is_space);
    let cleaned = text.replacen(',', ".", 1);
    let parts: Vec<&str> = cleaned.split(':').collect();
    match parts.len() {
        3 => {
            jsnum::number(parts[0]) * 3600.0
                + jsnum::number(parts[1]) * 60.0
                + jsnum::parse_float(parts[2])
        }
        2 => jsnum::number(parts[0]) * 60.0 + jsnum::parse_float(parts[1]),
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timestamp_is_read_in_seconds() {
        assert_eq!(timestamp("00:01:23,456"), 83.456);
        assert_eq!(timestamp("01:00:00,000"), 3600.0);
        assert_eq!(timestamp("02:03.500"), 123.5);
    }

    #[test]
    fn a_timestamp_that_is_not_one_is_not_a_number() {
        assert!(timestamp("").is_nan());
        assert!(timestamp("nonsense").is_nan());
        assert!(timestamp("00:00:00:00").is_nan());
    }

    #[test]
    fn a_file_parses_into_cues() {
        let srt = "1\n00:00:01,000 --> 00:00:03,000\nThe mill stood at the northern end.\n\n\
                   2\n00:00:03,000 --> 00:00:05,500\nIts wheel was undershot,\nwhich was wasteful.\n";
        let cues = parse(srt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start, 1.0);
        assert_eq!(cues[0].end, 3.0);
        assert_eq!(cues[0].text, "The mill stood at the northern end.");
        assert_eq!(cues[1].text, "Its wheel was undershot, which was wasteful.");
    }

    #[test]
    fn a_block_with_no_arrow_is_skipped() {
        assert!(parse("WEBVTT\n\nsome preamble\n").is_empty());
    }

    #[test]
    fn a_block_with_unreadable_times_is_dropped_whole() {
        let srt = "1\nnonsense --> also nonsense\ntext\n\n\
                   2\n00:00:03,000 --> 00:00:05,000\nkept\n";
        let cues = parse(srt);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "kept");
    }

    #[test]
    fn a_block_with_no_text_is_dropped() {
        assert!(parse("1\n00:00:01,000 --> 00:00:03,000\n[Music]\n").is_empty());
    }

    #[test]
    fn several_blank_lines_still_separate_one_block_from_the_next() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\none\n\n\n\n\
                   2\n00:00:02,000 --> 00:00:03,000\ntwo\n";
        assert_eq!(parse(srt).len(), 2);
    }

    #[test]
    fn webvtt_timing_and_settings_read_too() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000 line:0 position:20%\nThe mill.\n";
        let cues = parse(vtt);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].start, 1.0);
        assert_eq!(cues[0].end, 3.0);
    }
}
