//! Collapsing the repetition automatic captioning produces.

use crate::cue::Cue;

/// Merge consecutive cues whose text is identical, keeping the earliest start
/// and the latest end.
///
/// Automatic captioning emits a rolling window: the same line appears in cue
/// after cue as the window slides, so a ten-minute stretch of speech can
/// arrive as three times as many cues as there are sentences. Indexed as they
/// come, every one of those is a separate hit for the same words at almost
/// the same timestamp, which buries everything else in a result list.
pub fn merge_repeats(cues: &[Cue]) -> Vec<Cue> {
    let mut merged: Vec<Cue> = Vec::with_capacity(cues.len());
    for cue in cues {
        match merged.last_mut() {
            Some(previous) if previous.text == cue.text => {
                previous.end = previous.end.max(cue.end);
            }
            _ => merged.push(cue.clone()),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeated_line_becomes_one_cue_spanning_both() {
        let cues = [
            Cue::new(0.0, 2.0, "the mill stood"),
            Cue::new(1.0, 3.0, "the mill stood"),
            Cue::new(3.0, 4.0, "at the northern end"),
        ];
        let merged = merge_repeats(&cues);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start, 0.0);
        assert_eq!(merged[0].end, 3.0);
        assert_eq!(merged[1].text, "at the northern end");
    }

    #[test]
    fn a_line_that_comes_back_later_is_kept_separately() {
        let cues = [
            Cue::new(0.0, 1.0, "yes"),
            Cue::new(1.0, 2.0, "no"),
            Cue::new(2.0, 3.0, "yes"),
        ];
        assert_eq!(merge_repeats(&cues).len(), 3);
    }

    #[test]
    fn an_end_that_goes_backwards_does_not_shorten_the_merged_cue() {
        let cues = [Cue::new(0.0, 9.0, "x"), Cue::new(1.0, 2.0, "x")];
        let merged = merge_repeats(&cues);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].end, 9.0);
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(merge_repeats(&[]).is_empty());
    }
}
