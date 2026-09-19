//! Turns a transcript into topic chapters.
//!
//! No model and no network: the text is hashed into a small lexical vector a
//! window at a time, and a topic boundary is where two consecutive windows
//! stop resembling each other. It is crude, and it is also what makes a
//! three-hour recording navigable — the chapters it finds are the marks a
//! scrubber needs and the spans a summariser should be given one at a time.
//!
//! Every constant here is a shape decision about recorded speech, and each
//! one carries the reason it has the value it has.

pub mod segment;
pub mod title;
pub mod vector;
pub mod window;

use cuekit::Cue;

pub use segment::Segment;
pub use window::Window;

/// How long a window of transcript is, in seconds.
///
/// Long enough that a window has a subject, short enough that a boundary
/// lands within a minute of the real one.
pub const WINDOW_SECONDS: f64 = 75.0;

/// No chapter may be shorter than this.
///
/// A chapter mark every two minutes is not navigation, it is noise.
pub const MIN_SEGMENT_SECONDS: f64 = 8.0 * 60.0;

/// How many chapters a recording gets, as a function of how long it is.
pub const TARGET_SECONDS_PER_CHAPTER: f64 = 15.0 * 60.0;
/// Fewest chapters, whatever the length.
pub const MIN_CHAPTERS: usize = 4;
/// Most chapters, whatever the length.
pub const MAX_CHAPTERS: usize = 20;

/// How much of a chapter's text is kept as its excerpt, in characters.
pub const EXCERPT_CHARS: usize = 900;

/// Cut a transcript into chapters.
///
/// `duration` is how long the recording is. It is taken rather than derived
/// because a transcript often stops before the recording does — music at the
/// end, or a speaker who stopped talking — and a chapter that claims to run
/// to the end of the transcript rather than the end of the file is a chapter
/// mark that lands in the wrong place.
pub fn segment(cues: &[Cue], duration: f64) -> Vec<Segment> {
    let windows = window::windows(cues, WINDOW_SECONDS);
    if windows.is_empty() {
        return Vec::new();
    }
    let vectors: Vec<Vec<f64>> = windows
        .iter()
        .map(|window| vector::lexical(&window.text, vector::DIMENSIONS))
        .collect();
    let similarities = vector::consecutive_cosine(&vectors);
    let boundaries =
        segment::boundary_times(&windows, &similarities, duration, MIN_SEGMENT_SECONDS, None);
    segment::from_boundaries(&windows, &boundaries, duration)
}

/// How many chapters a recording of this length should have.
pub fn target_chapter_count(duration: f64) -> usize {
    let target = round_half_to_even(duration / TARGET_SECONDS_PER_CHAPTER);
    (target as usize).clamp(MIN_CHAPTERS, MAX_CHAPTERS)
}

/// Round half to even, which is what the Python this was ported from does
/// and what Rust's own `round` does not: `round(2.5)` is 2 there and 3 here,
/// and a recording exactly two and a half chapters long would otherwise get a
/// different number of them.
fn round_half_to_even(value: f64) -> f64 {
    let rounded = value.round();
    if (value - value.trunc()).abs() == 0.5 && rounded % 2.0 != 0.0 {
        rounded - value.signum()
    } else {
        rounded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_goes_to_even_on_a_half() {
        assert_eq!(round_half_to_even(0.5), 0.0);
        assert_eq!(round_half_to_even(1.5), 2.0);
        assert_eq!(round_half_to_even(2.5), 2.0);
        assert_eq!(round_half_to_even(3.5), 4.0);
        assert_eq!(round_half_to_even(-2.5), -2.0);
        assert_eq!(round_half_to_even(2.4), 2.0);
        assert_eq!(round_half_to_even(2.6), 3.0);
    }

    #[test]
    fn the_chapter_count_follows_the_length_within_bounds() {
        assert_eq!(target_chapter_count(0.0), MIN_CHAPTERS);
        assert_eq!(target_chapter_count(60.0 * 60.0), MIN_CHAPTERS);
        assert_eq!(target_chapter_count(90.0 * 60.0), 6);
        assert_eq!(target_chapter_count(100.0 * 60.0 * 60.0), MAX_CHAPTERS);
    }

    #[test]
    fn a_transcript_with_no_cues_has_no_chapters() {
        assert!(segment(&[], 3600.0).is_empty());
    }

    #[test]
    fn a_long_transcript_is_cut_into_several_chapters() {
        // Two subjects, an hour each, with nothing in common lexically.
        let mut cues = Vec::new();
        let mut at = 0.0;
        for round in 0..2 {
            let words = if round == 0 {
                "the mill stood at the northern end where the fall was steepest"
            } else {
                "parliament dissolved itself again amid furious objections from every bench"
            };
            while at < (round as f64 + 1.0) * 3600.0 {
                cues.push(Cue::new(at, at + 5.0, words));
                at += 5.0;
            }
        }
        let found = segment(&cues, at);
        assert!(found.len() >= 2, "{} chapters", found.len());
        assert_eq!(found[0].start, 0.0);
        assert_eq!(found.last().expect("a chapter").end, at);
        for pair in found.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "chapters must not leave a gap");
        }
    }
}
