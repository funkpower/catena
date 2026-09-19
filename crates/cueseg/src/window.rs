//! Gathering cues into windows long enough to have a subject.

use std::sync::LazyLock;

use cuekit::Cue;
use regex::Regex;

/// Runs of whitespace, collapsed so that a window's text is one paragraph.
static SPACE_RUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\s\x{1c}-\x{1f}]+").expect("a valid pattern"));

/// A stretch of transcript treated as one unit of subject matter.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    /// When the window starts.
    pub start: f64,
    /// When it ends.
    pub end: f64,
    /// Everything said in it, on one line.
    pub text: String,
}

/// Gather cues into windows of about `seconds` each.
///
/// A window closes when a cue starts more than `seconds` after the window
/// began — not when it would overrun, so a window is always at least as long
/// as the budget and never cuts a cue in half.
pub fn windows(cues: &[Cue], seconds: f64) -> Vec<Window> {
    let Some(first) = cues.first() else {
        return Vec::new();
    };

    let mut windows = Vec::new();
    let mut start = first.start;
    let mut end = first.end;
    let mut parts: Vec<&str> = Vec::new();

    for cue in cues {
        if !parts.is_empty() && cue.start - start >= seconds {
            flush(&mut windows, start, end, &parts);
            parts.clear();
        }
        if parts.is_empty() {
            start = cue.start;
        }
        parts.push(&cue.text);
        end = end.max(cue.end);
    }
    flush(&mut windows, start, end, &parts);
    windows
}

fn flush(windows: &mut Vec<Window>, start: f64, end: f64, parts: &[&str]) {
    let joined = parts.join(" ");
    let text = SPACE_RUN.replace_all(&joined, " ");
    let text = text.trim_matches(textkit::is_space);
    if !text.is_empty() {
        windows.push(Window {
            start,
            end,
            text: text.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f64, end: f64, text: &str) -> Cue {
        Cue::new(start, end, text)
    }

    #[test]
    fn cues_within_the_budget_make_one_window() {
        let cues = [cue(0.0, 5.0, "one"), cue(5.0, 10.0, "two")];
        let found = windows(&cues, 75.0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "one two");
        assert_eq!(found[0].start, 0.0);
        assert_eq!(found[0].end, 10.0);
    }

    #[test]
    fn a_window_closes_when_a_cue_starts_past_the_budget() {
        let cues = [
            cue(0.0, 5.0, "one"),
            cue(74.0, 76.0, "two"),
            cue(80.0, 85.0, "three"),
        ];
        let found = windows(&cues, 75.0);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].text, "one two");
        assert_eq!(found[1].text, "three");
        assert_eq!(found[1].start, 80.0);
    }

    #[test]
    fn an_end_time_that_goes_backwards_does_not_shorten_a_window() {
        let cues = [cue(0.0, 90.0, "one"), cue(10.0, 20.0, "two")];
        assert_eq!(windows(&cues, 75.0)[0].end, 90.0);
    }

    #[test]
    fn whitespace_inside_a_window_is_collapsed() {
        let cues = [cue(0.0, 1.0, "one\n\n  two"), cue(1.0, 2.0, "  three ")];
        assert_eq!(windows(&cues, 75.0)[0].text, "one two three");
    }

    #[test]
    fn a_window_of_nothing_but_whitespace_is_dropped() {
        let cues = [cue(0.0, 1.0, "   "), cue(100.0, 101.0, "kept")];
        let found = windows(&cues, 75.0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "kept");
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(windows(&[], 75.0).is_empty());
    }
}
