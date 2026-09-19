//! Choosing where the chapters begin, and cutting them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::window::Window;
use crate::{EXCERPT_CHARS, MAX_CHAPTERS, title};

/// One topic chapter of a recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Segment {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// Seconds from the start of the recording.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub start: f64,
    /// Seconds from the start of the recording.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub end: f64,
    /// A title taken from the chapter's own first sentence.
    ///
    /// Extractive and unembarrassed about it. A model can write a better one
    /// later; this is what makes the chapter usable before one has.
    pub title: String,
    /// The opening of the chapter, for a summariser or a reader.
    pub excerpt: String,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Segment {
    fn version(&self) -> u32 {
        self.v
    }
}

/// Where the chapters begin, in seconds.
///
/// A boundary is a valley: a pair of windows that resemble each other less
/// than their neighbours do, and less than the recording's own average by
/// half a standard deviation. Valleys are taken deepest first, each one at
/// least a chapter's length from the ones already taken.
pub fn boundary_times(
    windows: &[Window],
    similarities: &[f64],
    duration: f64,
    min_segment: f64,
    target_count: Option<usize>,
) -> Vec<f64> {
    let target = target_count.unwrap_or_else(|| crate::target_chapter_count(duration));
    if windows.len() < 2 {
        return Vec::new();
    }

    let smoothed = moving_average(similarities, 1);
    let average = if smoothed.is_empty() {
        0.0
    } else {
        smoothed.iter().sum::<f64>() / smoothed.len() as f64
    };
    let deviation = if smoothed.len() < 2 {
        0.0
    } else {
        let sum: f64 = smoothed.iter().map(|value| (value - average).powi(2)).sum();
        (sum / smoothed.len() as f64).sqrt()
    };
    // A recording whose windows all resemble each other equally has no
    // deviation to work from, so a fixed spread stands in for one.
    let spread = if deviation == 0.0 { 0.15 } else { deviation };
    let threshold = average - 0.5 * spread;

    let mut valleys: Vec<(usize, f64)> = Vec::new();
    for (at, value) in smoothed.iter().enumerate() {
        let before = if at > 0 {
            smoothed[at - 1]
        } else {
            value + 1.0
        };
        let after = if at < smoothed.len() - 1 {
            smoothed[at + 1]
        } else {
            value + 1.0
        };
        if *value <= before && *value <= after && *value < threshold {
            valleys.push((at, *value));
        }
    }
    // Stable, so that two equally deep valleys are taken in the order they
    // occur in the recording.
    valleys.sort_by(|a, b| a.1.total_cmp(&b.1));

    let needed = target.saturating_sub(1);
    let time_of =
        |gap: usize| -> f64 { windows.get(gap + 1).map_or(duration, |window| window.start) };

    let mut chosen: Vec<usize> = Vec::new();
    for (at, _) in valleys {
        let time = time_of(at);
        if time < min_segment || duration - time < min_segment {
            continue;
        }
        if chosen
            .iter()
            .any(|taken| (time_of(*taken) - time).abs() < min_segment)
        {
            continue;
        }
        chosen.push(at);
        if chosen.len() >= needed {
            break;
        }
    }

    // Nothing stood out, but the recording is long enough to want chapters
    // anyway: fall back to even spacing, which is better than one chapter of
    // three hours.
    if chosen.is_empty() && duration >= min_segment * 2.0 {
        let count = needed.max(1);
        return (1..=count)
            .map(|k| duration * k as f64 / (count + 1) as f64)
            .filter(|time| *time >= min_segment && duration - *time >= min_segment)
            .collect();
    }

    let mut times: Vec<f64> = chosen.iter().map(|at| time_of(*at)).collect();
    times.sort_by(f64::total_cmp);

    // A chapter that runs too long is no more navigable than no chapter at
    // all, so an over-long span is divided evenly whether or not the text
    // suggested a break there.
    let longest = (min_segment * 2.5).max(duration / MAX_CHAPTERS as f64);
    let mut ends = vec![0.0];
    ends.extend(times.iter().copied());
    ends.push(duration);

    let mut extra = Vec::new();
    for pair in ends.windows(2) {
        let span = pair[1] - pair[0];
        if span <= longest {
            continue;
        }
        let parts = (span / longest).ceil() as usize;
        for part in 1..parts {
            extra.push(pair[0] + span * part as f64 / parts as f64);
        }
    }
    times.extend(extra);
    times.sort_by(f64::total_cmp);
    times
}

/// Cut the windows into chapters at the given times.
pub fn from_boundaries(windows: &[Window], boundaries: &[f64], duration: f64) -> Vec<Segment> {
    if windows.is_empty() {
        return Vec::new();
    }
    let mut cuts = vec![0.0];
    cuts.extend(
        boundaries
            .iter()
            .copied()
            .filter(|time| *time > 0.0 && *time < duration),
    );
    cuts.push(duration);

    // A cut within a second of the one before it would make a chapter nobody
    // can click on. Compared against the unfiltered list, as the original
    // does, so a run of near-identical cuts collapses to its first.
    let unique: Vec<f64> = cuts
        .iter()
        .enumerate()
        .filter(|(at, time)| *at == 0 || **time - cuts[at - 1] > 1.0)
        .map(|(_, time)| *time)
        .collect();

    let mut segments = Vec::new();
    for pair in unique.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let body: Vec<&str> = windows
            .iter()
            .filter(|window| window.start < end && window.end > start)
            .map(|window| window.text.as_str())
            .collect();
        let excerpt = excerpt_of(&body.join(" "));
        if excerpt.is_empty() {
            continue;
        }
        segments.push(Segment {
            v: corpus_core::V,
            start,
            end,
            title: title::extract(&excerpt),
            excerpt,
        });
    }
    segments
}

/// Smooth a series by averaging each value with its neighbours, so that one
/// unusual window does not read as a topic change.
fn moving_average(values: &[f64], radius: usize) -> Vec<f64> {
    (0..values.len())
        .map(|at| {
            let from = at.saturating_sub(radius);
            let to = (at + radius + 1).min(values.len());
            let slice = &values[from..to];
            if slice.is_empty() {
                0.0
            } else {
                slice.iter().sum::<f64>() / slice.len() as f64
            }
        })
        .collect()
}

/// The opening of a chapter, cut at a word boundary where one is close
/// enough to the limit to be worth using.
fn excerpt_of(text: &str) -> String {
    if text.chars().count() <= EXCERPT_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(EXCERPT_CHARS).collect();
    match head.rfind(' ') {
        Some(at) if head[..at].chars().count() > 200 => head[..at].trim().to_string(),
        _ => head,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(start: f64, end: f64, text: &str) -> Window {
        Window {
            start,
            end,
            text: text.to_string(),
        }
    }

    #[test]
    fn a_single_window_has_no_boundaries() {
        let windows = [window(0.0, 10.0, "one")];
        assert!(boundary_times(&windows, &[], 10.0, 480.0, None).is_empty());
    }

    #[test]
    fn a_boundary_lands_where_the_subject_changes() {
        let windows: Vec<Window> = (0..40)
            .map(|at| {
                let start = at as f64 * 75.0;
                window(start, start + 75.0, "text")
            })
            .collect();
        // Similar throughout except one deep valley in the middle.
        let mut similarities = vec![0.9; 39];
        similarities[19] = 0.1;
        let found = boundary_times(&windows, &similarities, 3000.0, 1200.0, Some(2));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], windows[20].start);
    }

    #[test]
    fn a_chapter_that_would_run_too_long_is_divided_anyway() {
        let windows: Vec<Window> = (0..40)
            .map(|at| {
                let start = at as f64 * 75.0;
                window(start, start + 75.0, "text")
            })
            .collect();
        let mut similarities = vec![0.9; 39];
        similarities[19] = 0.1;
        // The longest a chapter may run here is 1200 seconds, so each half of
        // a 3000-second recording split once in the middle is still too long.
        let found = boundary_times(&windows, &similarities, 3000.0, 480.0, Some(2));
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found.contains(&windows[20].start));
        let mut ends = vec![0.0];
        ends.extend(found.iter().copied());
        ends.push(3000.0);
        for pair in ends.windows(2) {
            assert!(pair[1] - pair[0] <= 1200.0, "{:?} runs too long", pair);
        }
    }

    #[test]
    fn a_recording_with_no_valleys_is_cut_evenly() {
        let windows: Vec<Window> = (0..40)
            .map(|at| {
                let start = at as f64 * 75.0;
                window(start, start + 75.0, "text")
            })
            .collect();
        let found = boundary_times(&windows, &vec![0.9; 39], 3000.0, 480.0, Some(3));
        assert_eq!(found.len(), 2);
        assert!(found[0] < found[1]);
    }

    #[test]
    fn chapters_meet_end_to_end_and_cover_the_recording() {
        let windows = [
            window(
                0.0,
                1000.0,
                "the mill stood at the northern end of the valley",
            ),
            window(
                1000.0,
                2000.0,
                "parliament dissolved itself amid objections",
            ),
        ];
        let found = from_boundaries(&windows, &[1000.0], 2000.0);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].start, 0.0);
        assert_eq!(found[0].end, 1000.0);
        assert_eq!(found[1].start, 1000.0);
        assert_eq!(found[1].end, 2000.0);
    }

    #[test]
    fn a_boundary_a_moment_after_another_is_dropped() {
        let windows = [window(0.0, 2000.0, "text enough to excerpt")];
        let found = from_boundaries(&windows, &[1000.0, 1000.5], 2000.0);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn a_boundary_outside_the_recording_is_ignored() {
        let windows = [window(0.0, 2000.0, "text enough to excerpt")];
        let found = from_boundaries(&windows, &[-5.0, 5000.0], 2000.0);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn an_excerpt_is_cut_at_a_word() {
        let text = "word ".repeat(400);
        let excerpt = excerpt_of(&text);
        assert!(excerpt.chars().count() <= EXCERPT_CHARS);
        assert!(!excerpt.ends_with(' '));
        assert!(text.starts_with(&excerpt));
    }

    #[test]
    fn a_short_chapter_keeps_all_its_text() {
        assert_eq!(excerpt_of("short"), "short");
    }
}
