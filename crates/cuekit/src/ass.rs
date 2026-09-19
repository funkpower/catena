//! Advanced SubStation Alpha, which is a spreadsheet pretending to be a
//! subtitle file.
//!
//! Events are rows under a `Format:` line that names the columns, so the
//! column order has to be read rather than assumed — a file written by one
//! tool puts the text last after nine fields, and one written by another puts
//! it last after eight.

use std::sync::LazyLock;

use regex::Regex;

use crate::cue::Cue;
use crate::text;

/// An override block: `{\an8}`, `{\pos(120,400)}`, `{\i1}`.
static OVERRIDE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{[^}]*\}").expect("valid"));

/// A drawing command block, which is a shape rather than speech.
static DRAWING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\{[^}]*\\p[1-9]").expect("valid"));

/// Parse an ASS or SSA file.
pub fn parse(content: &str) -> Vec<Cue> {
    let mut columns: Option<Vec<String>> = None;
    let mut in_events = false;
    let mut cues = Vec::new();

    for line in content.split('\n') {
        let line = line.trim_matches(textkit::is_space);
        if line.starts_with('[') {
            in_events = line.eq_ignore_ascii_case("[events]");
            continue;
        }
        if !in_events {
            continue;
        }
        if let Some(format) = line.strip_prefix("Format:") {
            columns = Some(
                format
                    .split(',')
                    .map(|name| name.trim_matches(textkit::is_space).to_lowercase())
                    .collect(),
            );
            continue;
        }
        let Some(event) = line.strip_prefix("Dialogue:") else {
            continue;
        };
        let Some(columns) = columns.as_ref() else {
            continue;
        };
        if let Some(cue) = dialogue(event, columns) {
            cues.push(cue);
        }
    }
    cues
}

fn dialogue(event: &str, columns: &[String]) -> Option<Cue> {
    let start_at = columns.iter().position(|name| name == "start")?;
    let end_at = columns.iter().position(|name| name == "end")?;
    let text_at = columns.iter().position(|name| name == "text")?;

    // The text field is last and may itself contain commas, so the row is
    // split only as many times as there are columns before it.
    let mut fields: Vec<&str> = event.splitn(columns.len(), ',').collect();
    if fields.len() < columns.len() {
        return None;
    }
    let text = if text_at == columns.len() - 1 {
        fields.pop().unwrap_or("")
    } else {
        fields.get(text_at).copied().unwrap_or("")
    };

    let start = timestamp(fields.get(start_at)?.trim_matches(textkit::is_space));
    let end = timestamp(fields.get(end_at)?.trim_matches(textkit::is_space));
    if !start.is_finite() || !end.is_finite() {
        return None;
    }

    // A drawing command is a shape, not speech, and reads as a string of
    // coordinates if it is kept.
    if DRAWING.is_match(text) {
        return None;
    }
    let text = OVERRIDE.replace_all(text, "");
    let text = text
        .replace("\\N", " ")
        .replace("\\n", " ")
        .replace("\\h", " ");
    let text = text::clean(&text);
    (!text.is_empty()).then(|| Cue::new(start, end, text))
}

/// Seconds from an `H:MM:SS.cc` timestamp.
pub fn timestamp(text: &str) -> f64 {
    let parts: Vec<&str> = text.split(':').collect();
    match parts.as_slice() {
        [hours, minutes, seconds] => {
            crate::jsnum::number(hours) * 3600.0
                + crate::jsnum::number(minutes) * 60.0
                + crate::jsnum::parse_float(seconds)
        }
        [minutes, seconds] => {
            crate::jsnum::number(minutes) * 60.0 + crate::jsnum::parse_float(seconds)
        }
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "[Script Info]\n\
        Title: A Recording\n\
        \n\
        [V4+ Styles]\n\
        Format: Name, Fontname\n\
        Style: Default,Arial\n\
        \n\
        [Events]\n\
        Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
        Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,The mill stood at the northern end.\n\
        Dialogue: 0,0:00:03.00,0:00:05.50,Default,,0,0,0,,{\\an8}Its wheel was undershot,\\Nwhich was wasteful.\n";

    #[test]
    fn events_become_cues() {
        let cues = parse(FILE);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start, 1.0);
        assert_eq!(cues[0].end, 3.0);
        assert_eq!(cues[0].text, "The mill stood at the northern end.");
    }

    #[test]
    fn override_blocks_and_line_breaks_come_out() {
        let cues = parse(FILE);
        assert_eq!(cues[1].text, "Its wheel was undershot, which was wasteful.");
    }

    #[test]
    fn a_comma_in_the_text_is_not_a_column_break() {
        let cues = parse(FILE);
        assert!(cues[1].text.contains(','), "{:?}", cues[1].text);
    }

    #[test]
    fn a_drawing_command_is_not_speech() {
        let file = FILE.to_string()
            + "Dialogue: 0,0:00:06.00,0:00:07.00,Default,,0,0,0,,{\\p1}m 0 0 l 100 0 100 100\n";
        assert_eq!(parse(&file).len(), 2);
    }

    #[test]
    fn a_row_outside_the_events_section_is_ignored() {
        let file = "[Script Info]\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,no\n";
        assert!(parse(file).is_empty());
    }

    #[test]
    fn a_column_order_of_its_own_is_honoured() {
        let file = "[Events]\n\
            Format: Start, End, Text\n\
            Dialogue: 0:00:01.00,0:00:02.00,Reordered, with a comma\n";
        let cues = parse(file);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "Reordered, with a comma");
    }

    #[test]
    fn a_timestamp_is_read_in_seconds() {
        assert_eq!(timestamp("0:00:01.50"), 1.5);
        assert_eq!(timestamp("1:02:03.04"), 3723.04);
        assert!(timestamp("rubbish").is_nan());
    }
}
