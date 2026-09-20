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
    segment_with(cues, duration, None)
}

/// The same, given vectors somebody else computed for the windows.
///
/// The hashed bag of words this falls back on asks only "are these two
/// stretches about the same thing", and answers it from repeated words. A
/// real embedding answers the same question far better, because it knows
/// that a stretch about *ships* and a stretch about *vessels* are about one
/// thing, which no amount of word counting will tell you.
///
/// One vector per window, in order. [`windows_of`] gives the windows to embed
/// so a caller can produce them without reimplementing the pass. A slice of
/// the wrong length is ignored rather than trusted — a caller that embedded
/// a different set of windows is asking a question about some other
/// recording — and the lexical route stands in.
pub fn segment_with(cues: &[Cue], duration: f64, embeddings: Option<&[Vec<f64>]>) -> Vec<Segment> {
    let windows = window::windows(cues, WINDOW_SECONDS);
    if windows.is_empty() {
        return Vec::new();
    }

    let lexical: Vec<Vec<f64>>;
    let vectors: &[Vec<f64>] = match embeddings {
        Some(given) if given.len() == windows.len() => given,
        _ => {
            lexical = windows
                .iter()
                .map(|window| vector::lexical(&window.text, vector::DIMENSIONS))
                .collect();
            &lexical
        }
    };

    let similarities = vector::consecutive_cosine(vectors);
    let boundaries =
        segment::boundary_times(&windows, &similarities, duration, MIN_SEGMENT_SECONDS, None);
    segment::from_boundaries(&windows, &boundaries, duration)
}

/// The windows a recording is measured in, so a caller can embed them.
///
/// The text of each is what [`segment_with`] expects a vector for, and the
/// order is the order the vectors must arrive in.
pub fn windows_of(cues: &[Cue]) -> Vec<window::Window> {
    window::windows(cues, WINDOW_SECONDS)
}

/// How many chapters a recording of this length should have.
pub fn target_chapter_count(duration: f64) -> usize {
    let target = textkit::round_half_to_even(duration / TARGET_SECONDS_PER_CHAPTER);
    (target as usize).clamp(MIN_CHAPTERS, MAX_CHAPTERS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transcript that talks about two different things, in two halves.
    fn two_halves() -> Vec<Cue> {
        (0..1200)
            .map(|i| Cue {
                v: corpus_core::V,
                start: i as f64 * 5.0,
                end: i as f64 * 5.0 + 5.0,
                text: match i < 600 {
                    true => "the mill stood at the northern end of the valley".into(),
                    false => "parliament dissolved itself again amid objections".into(),
                },
            })
            .collect()
    }

    #[test]
    fn supplied_vectors_are_used_instead_of_hashing_the_words() {
        let cues = two_halves();
        let windows = windows_of(&cues);
        // Vectors that say every window is about the same thing. Nothing the
        // words say can produce this, so if the answer changes, they were used.
        let flat: Vec<Vec<f64>> = windows.iter().map(|_| vec![1.0, 0.0]).collect();
        assert_ne!(
            segment_with(&cues, 6000.0, Some(&flat)),
            segment_with(&cues, 6000.0, None),
            "supplied vectors made no difference"
        );
    }

    #[test]
    fn the_windows_to_embed_line_up_with_what_is_expected() {
        let cues = two_halves();
        let windows = windows_of(&cues);
        let vectors: Vec<Vec<f64>> = windows
            .iter()
            .map(|w| vector::lexical(&w.text, vector::DIMENSIONS))
            .collect();
        // Handing back exactly what the lexical route would have computed
        // must give exactly what the lexical route gives.
        assert_eq!(
            segment_with(&cues, 6000.0, Some(&vectors)),
            segment(&cues, 6000.0)
        );
    }

    #[test]
    fn the_wrong_number_of_vectors_is_ignored_rather_than_trusted() {
        // A caller that embedded some other recording's windows is asking a
        // question about that recording, not this one.
        let cues = two_halves();
        let too_few = vec![vec![1.0, 0.0]; 2];
        assert_eq!(
            segment_with(&cues, 6000.0, Some(&too_few)),
            segment(&cues, 6000.0)
        );
        assert_eq!(
            segment_with(&cues, 6000.0, Some(&[])),
            segment(&cues, 6000.0)
        );
    }

    #[test]
    fn segment_is_still_the_lexical_route() {
        let cues = two_halves();
        assert_eq!(segment(&cues, 6000.0), segment_with(&cues, 6000.0, None));
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
