//! The cue model everything timed shares, and the subtitle formats it is
//! read from.
//!
//! Three formats go in — SubRip, WebVTT and SubStation Alpha — and one cue
//! stream comes out. Everything downstream of this works on cues and never
//! has to know which of the three a recording arrived with.
//!
//! The SubRip reader is a port, quirks kept, of the parser a podcast archive
//! was indexed with; the other two are new, and are written to the same
//! shape. Where the port deliberately differs, `parity/KNOWN.md` says so.

pub mod ass;
pub mod cue;
pub mod dedup;
pub mod jsnum;
pub mod srt;
pub mod text;

pub use cue::Cue;
pub use dedup::merge_repeats;

/// Which subtitle format a file is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// SubRip.
    SubRip,
    /// WebVTT, which this reads with the SubRip reader.
    WebVtt,
    /// SubStation Alpha, advanced or otherwise.
    SubStationAlpha,
}

impl Format {
    /// What this format is called.
    pub fn name(self) -> &'static str {
        match self {
            Format::SubRip => "srt",
            Format::WebVtt => "vtt",
            Format::SubStationAlpha => "ass",
        }
    }
}

/// Work out which format a file is, from its content rather than its name.
///
/// A subtitle file arrives named for the format someone expected it to be in.
/// The content says what it actually is, and the three are easy to tell
/// apart: WebVTT announces itself, SubStation Alpha has sections in square
/// brackets, and SubRip has neither.
pub fn detect(content: &str) -> Format {
    let head = content.trim_start_matches(|c: char| textkit::is_space(c) || c == '\u{feff}');
    if head.starts_with("WEBVTT") {
        return Format::WebVtt;
    }
    for line in head.split('\n').take(40) {
        let line = line.trim_matches(textkit::is_space);
        if line.starts_with("[Script Info]") || line.eq_ignore_ascii_case("[events]") {
            return Format::SubStationAlpha;
        }
        if line.starts_with("Dialogue:") || line.starts_with("ScriptType:") {
            return Format::SubStationAlpha;
        }
    }
    Format::SubRip
}

/// Read a subtitle file of any supported format into cues.
pub fn parse(content: &str) -> Vec<Cue> {
    parse_as(content, detect(content))
}

/// Read a subtitle file that is known to be in `format`.
pub fn parse_as(content: &str, format: Format) -> Vec<Cue> {
    match format {
        Format::SubRip | Format::WebVtt => srt::parse(content),
        Format::SubStationAlpha => ass::parse(content),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_recognised_by_what_is_in_it() {
        assert_eq!(
            detect("WEBVTT\n\n00:00.000 --> 00:01.000\nx"),
            Format::WebVtt
        );
        assert_eq!(detect("\u{feff}WEBVTT\n"), Format::WebVtt);
        assert_eq!(detect("[Script Info]\nTitle: x\n"), Format::SubStationAlpha);
        assert_eq!(
            detect("[Events]\nFormat: Start, End, Text\n"),
            Format::SubStationAlpha
        );
        assert_eq!(
            detect("1\n00:00:01,000 --> 00:00:02,000\nx\n"),
            Format::SubRip
        );
        assert_eq!(detect(""), Format::SubRip);
    }

    #[test]
    fn all_three_formats_give_the_same_cue() {
        let srt = "1\n00:00:01,000 --> 00:00:03,000\nThe mill stood.\n";
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nThe mill stood.\n";
        let ass = "[Events]\nFormat: Start, End, Text\n\
                   Dialogue: 0:00:01.00,0:00:03.00,The mill stood.\n";
        let expected = vec![Cue::new(1.0, 3.0, "The mill stood.")];
        assert_eq!(parse(srt), expected);
        assert_eq!(parse(vtt), expected);
        assert_eq!(parse(ass), expected);
    }
}
