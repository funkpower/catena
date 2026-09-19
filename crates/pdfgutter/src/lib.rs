//! Two facing pages photographed as one landscape sheet, found and split.
//!
//! A book held open under a camera gives one wide image per shot, with two
//! pages on it. Everything downstream reads it as a single page: the columns
//! interleave, the reading order comes out scrambled, and the text that
//! results is worse than useless because it looks like text.
//!
//! Aspect ratio alone is not enough to say so — a foldout, a plate, a page
//! set in one narrow centred column are all genuinely wide. So the shape of
//! the ink is checked too: two blocks of text with a blank band down the
//! middle, on most of the pages sampled.
//!
//! Splitting is a change to the page boxes and nothing else. Nothing is
//! re-rendered, so a 250-sheet book becomes a 500-page book at no cost in
//! quality and almost none in size.

pub mod ink;
pub mod pdf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Above this width-to-height ratio a sheet is wide enough to hold two pages.
///
/// Two ordinary portrait pages side by side land near 1.36. A lone wide page
/// can sit anywhere, which is why the gutter is the real test.
pub const MIN_RATIO: f64 = 1.15;

/// How many of the sampled wide sheets must show a gutter before the whole
/// document is called two-up.
pub const MIN_GUTTER_FRACTION: f64 = 0.6;

/// How dark a pixel has to be to count as ink, from 0 black to 255 white.
pub const INK_THRESHOLD: u8 = 160;

/// What fraction of a column has to be ink before the column counts as text.
pub const TEXT_COLUMN_INK: f64 = 0.02;

/// How many pages are looked at, however long the document is.
pub const SAMPLE_SIZE: usize = 8;

/// What one page looks like.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Page {
    /// Which page it is, counting from one.
    pub number: usize,
    /// Its width divided by its height.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub ratio: f64,
    /// Whether a gutter was found. `None` when the page was too narrow to be
    /// worth looking at.
    pub gutter: Option<bool>,
}

/// What a document is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Verdict {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// The file the verdict is about.
    pub path: String,
    /// Whether it is two pages to a sheet.
    pub two_up: bool,
    /// How many pages it has.
    pub pages: usize,
    /// The middle ratio of the sampled pages.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub median_ratio: f64,
    /// What share of the sampled wide sheets showed a gutter.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub gutter_fraction: f64,
    /// The pages that were looked at.
    pub sampled: Vec<Page>,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Verdict {
    fn version(&self) -> u32 {
        self.v
    }
}

/// A page's width divided by its height, or zero for a page with no height.
pub fn aspect_ratio(width: f64, height: f64) -> f64 {
    match height {
        0.0 => 0.0,
        height => width / height,
    }
}

/// Which pages to look at: evenly spaced, and away from the ends.
///
/// Covers and end matter are single pages even in a book that was otherwise
/// shot two at a time, so sampling them would argue against a document that
/// plainly is two-up.
pub fn sample_indices(pages: usize) -> Vec<usize> {
    if pages <= 6 {
        return (0..pages).collect();
    }
    let (low, high) = (2, pages - 3);
    let step = ((high - low) / SAMPLE_SIZE).max(1);
    (low..=high).step_by(step).take(SAMPLE_SIZE).collect()
}

/// Whether a column ink profile has a blank band down the middle with text on
/// both sides of it.
///
/// The three bands are deliberately narrow and deliberately apart: the middle
/// eight per cent for the gutter, and a third of the width either side of it
/// for the text, leaving the outer tenth alone because a scan's edges carry
/// the shadow of the book's binding and the table it sat on.
pub fn has_gutter(ink: &[f64]) -> bool {
    let width = ink.len() as f64;
    let band = |from: f64, to: f64| {
        let (from, to) = ((from * width) as usize, (to * width) as usize);
        ink.get(from..to.min(ink.len())).unwrap_or(&[])
    };

    let centre = band(0.46, 0.54);
    let left = band(0.10, 0.44);
    let right = band(0.56, 0.90);
    if centre.is_empty() || left.is_empty() || right.is_empty() {
        return false;
    }

    let blank = centre.iter().copied().fold(f64::INFINITY, f64::min);
    blank < TEXT_COLUMN_INK && median(left) > TEXT_COLUMN_INK && median(right) > TEXT_COLUMN_INK
}

/// The middle value, averaging the two middle values of an even-length run.
///
/// The middle rather than the mean throughout, because one page of solid
/// plate or one blank leaf should not move the answer for a whole book.
pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    match sorted.len() % 2 {
        0 => (sorted[middle - 1] + sorted[middle]) / 2.0,
        _ => sorted[middle],
    }
}

/// Whether the sampled pages add up to a two-up document.
///
/// `gutters` holds one answer per sampled page that was wide enough to look
/// at, which is why it is usually shorter than `ratios`.
pub fn is_two_up(ratios: &[f64], gutters: &[bool]) -> bool {
    let median_ratio = median(ratios);
    median_ratio > MIN_RATIO && gutter_fraction(gutters) >= MIN_GUTTER_FRACTION
}

/// What share of the pages looked at showed a gutter.
pub fn gutter_fraction(gutters: &[bool]) -> f64 {
    match gutters.len() {
        0 => 0.0,
        len => gutters.iter().filter(|found| **found).count() as f64 / len as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile with text either side of a blank middle.
    fn two_pages() -> Vec<f64> {
        (0..1000)
            .map(|x| match x {
                x if (100..440).contains(&x) => 0.3,
                x if (560..900).contains(&x) => 0.3,
                _ => 0.0,
            })
            .collect()
    }

    #[test]
    fn two_columns_either_side_of_a_blank_band_is_a_gutter() {
        assert!(has_gutter(&two_pages()));
    }

    #[test]
    fn one_wide_block_of_text_is_not() {
        let solid: Vec<f64> = (0..1000).map(|_| 0.3).collect();
        assert!(!has_gutter(&solid));
    }

    #[test]
    fn a_narrow_centred_column_is_not_two_pages() {
        // The shape that makes aspect ratio alone useless: a wide sheet with
        // one column of text down the middle of it.
        let centred: Vec<f64> = (0..1000)
            .map(|x| match (400..600).contains(&x) {
                true => 0.3,
                false => 0.0,
            })
            .collect();
        assert!(!has_gutter(&centred));
    }

    #[test]
    fn a_blank_page_has_no_gutter_because_it_has_no_text() {
        assert!(!has_gutter(&[0.0; 1000]));
    }

    #[test]
    fn text_on_only_one_side_is_one_page_that_happens_to_be_wide() {
        let half: Vec<f64> = (0..1000)
            .map(|x| match (100..440).contains(&x) {
                true => 0.3,
                false => 0.0,
            })
            .collect();
        assert!(!has_gutter(&half));
    }

    #[test]
    fn a_gutter_narrower_than_the_band_it_is_looked_for_in_still_counts() {
        // Only one blank column is needed, not a blank band: scans are rarely
        // square to the camera and the gutter wanders.
        let mut ink = vec![0.3; 1000];
        ink[500] = 0.0;
        assert!(has_gutter(&ink));
    }

    #[test]
    fn a_profile_too_short_to_have_bands_is_not_a_gutter() {
        assert!(!has_gutter(&[]));
        assert!(!has_gutter(&[0.0, 1.0]));
    }

    #[test]
    fn short_documents_are_sampled_whole() {
        assert_eq!(sample_indices(0), Vec::<usize>::new());
        assert_eq!(sample_indices(1), [0]);
        assert_eq!(sample_indices(6), [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn longer_ones_skip_the_covers() {
        assert_eq!(sample_indices(7), [2, 3, 4]);
        assert_eq!(sample_indices(12), [2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(sample_indices(100), [2, 13, 24, 35, 46, 57, 68, 79]);
    }

    #[test]
    fn no_more_than_eight_pages_are_ever_looked_at() {
        for pages in 7..500 {
            assert!(sample_indices(pages).len() <= SAMPLE_SIZE, "{pages}");
        }
    }

    #[test]
    fn every_sampled_page_exists() {
        for pages in 0..500 {
            for index in sample_indices(pages) {
                assert!(index < pages, "{pages} has no page {index}");
            }
        }
    }

    #[test]
    fn a_page_with_no_height_has_no_ratio_rather_than_an_infinite_one() {
        assert_eq!(aspect_ratio(600.0, 0.0), 0.0);
        assert_eq!(aspect_ratio(1008.0, 720.0), 1.4);
    }

    #[test]
    fn the_middle_of_an_even_run_is_the_mean_of_the_two_middle_values() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
        assert_eq!(median(&[1.0, 2.0, 3.0]), 2.0);
        assert_eq!(median(&[]), 0.0);
    }

    #[test]
    fn a_document_needs_both_the_shape_and_the_gutter() {
        let wide = [1.4; 8];
        let narrow = [0.7; 8];
        assert!(is_two_up(&wide, &[true; 8]));
        assert!(!is_two_up(&narrow, &[true; 8]));
        assert!(!is_two_up(&wide, &[]));
        // Three of eight is not most of them.
        let mostly_not = [true, true, true, false, false, false, false, false];
        assert!(!is_two_up(&wide, &mostly_not));
    }

    #[test]
    fn a_document_only_just_over_the_line_still_counts() {
        // Six of ten is exactly the threshold, and the threshold is inclusive.
        let gutters = [
            true, true, true, true, true, true, false, false, false, false,
        ];
        assert_eq!(gutter_fraction(&gutters), 0.6);
        assert!(is_two_up(&[1.4; 10], &gutters));
    }
}
