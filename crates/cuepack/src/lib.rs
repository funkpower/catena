//! Packs cues into passages worth embedding.
//!
//! A cue is a few seconds of speech and half a sentence. An embedding of one
//! says almost nothing, and a search over them returns fragments. Packing
//! them into a minute or so of continuous speech gives a passage with enough
//! context to mean something, and — because the pack keeps the first cue's
//! start and the last cue's end — one that can still be cited to the second.

use corpus_core::Chunk;
use cuekit::Cue;

/// Where a passage is flushed, whichever comes first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budgets {
    /// Seconds of audio a passage covers.
    ///
    /// Seventy-five is about a paragraph of speech: long enough to hold an
    /// argument, short enough that a result points at a place in the
    /// recording rather than at a region of it.
    pub seconds: f64,
    /// Tokens a passage may hold, whatever the clock says.
    pub max_tokens: usize,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            seconds: 75.0,
            max_tokens: 512,
        }
    }
}

/// Pack consecutive cues into passages.
///
/// A passage is flushed when the window from its first cue's start to the
/// current cue's end reaches the seconds budget, or when adding the current
/// cue would take it past the token budget.
///
/// Immediately repeated lines are dropped rather than packed, because
/// automatic captioning repeats each line across overlapping windows and a
/// passage padded with its own echo embeds as something nobody said.
pub fn pack(cues: &[Cue], budgets: &Budgets) -> Vec<Chunk> {
    let mut passages: Vec<Chunk> = Vec::new();
    let mut buffered: Vec<&Cue> = Vec::new();
    let mut buffered_tokens = 0usize;
    let mut last_text: Option<&str> = None;

    for cue in cues {
        let text = cue.text.trim_matches(textkit::is_space);
        if text.is_empty() || Some(text) == last_text {
            continue;
        }
        last_text = Some(text);

        let tokens = textkit::count(text);
        let span = buffered
            .first()
            .map_or(0.0, |first: &&Cue| cue.end - first.start);
        if !buffered.is_empty()
            && (span >= budgets.seconds || buffered_tokens + tokens > budgets.max_tokens)
        {
            flush(&mut passages, &mut buffered, &mut buffered_tokens);
        }
        buffered.push(cue);
        buffered_tokens += tokens;
    }
    flush(&mut passages, &mut buffered, &mut buffered_tokens);
    passages
}

fn flush(passages: &mut Vec<Chunk>, buffered: &mut Vec<&Cue>, buffered_tokens: &mut usize) {
    if buffered.is_empty() {
        return;
    }
    let text = buffered
        .iter()
        .map(|cue| cue.text.trim_matches(textkit::is_space))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if !text.is_empty() {
        let tokens = textkit::count(&text);
        let mut chunk = Chunk::new(passages.len() as u32, text, tokens as u32);
        chunk.start_time = buffered.first().map(|cue| cue.start);
        chunk.end_time = buffered.last().map(|cue| cue.end);
        passages.push(chunk);
    }
    buffered.clear();
    *buffered_tokens = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f64, end: f64, text: &str) -> Cue {
        Cue::new(start, end, text)
    }

    #[test]
    fn cues_within_the_budgets_make_one_passage() {
        let cues = [
            cue(0.0, 5.0, "the mill stood"),
            cue(5.0, 10.0, "at the northern end"),
        ];
        let packed = pack(&cues, &Budgets::default());
        assert_eq!(packed.len(), 1);
        assert_eq!(packed[0].text, "the mill stood at the northern end");
        assert_eq!(packed[0].start_time, Some(0.0));
        assert_eq!(packed[0].end_time, Some(10.0));
        assert_eq!(packed[0].idx, 0);
    }

    #[test]
    fn a_passage_is_flushed_when_it_has_run_long_enough() {
        // The window is measured to the end of the cue being considered, so
        // the cue that trips the budget starts the next passage rather than
        // finishing this one.
        let cues = [
            cue(0.0, 40.0, "one"),
            cue(40.0, 80.0, "two"),
            cue(80.0, 90.0, "three"),
        ];
        let packed = pack(&cues, &Budgets::default());
        assert_eq!(packed.len(), 2);
        assert_eq!(packed[0].text, "one");
        assert_eq!(packed[0].end_time, Some(40.0));
        assert_eq!(packed[1].text, "two three");
        assert_eq!(packed[1].idx, 1);
    }

    #[test]
    fn a_passage_is_flushed_before_it_overruns_the_token_budget() {
        let budgets = Budgets {
            seconds: 1000.0,
            max_tokens: 6,
        };
        let cues = [
            cue(0.0, 1.0, "one two three"),
            cue(1.0, 2.0, "four five six"),
            cue(2.0, 3.0, "seven eight nine"),
        ];
        let packed = pack(&cues, &budgets);
        assert_eq!(packed.len(), 2);
        for passage in &packed {
            assert!(passage.tokens <= 6, "{} tokens", passage.tokens);
        }
    }

    #[test]
    fn an_immediately_repeated_line_is_dropped() {
        let cues = [
            cue(0.0, 2.0, "the mill stood"),
            cue(1.0, 3.0, "the mill stood"),
            cue(3.0, 4.0, "at the northern end"),
        ];
        let packed = pack(&cues, &Budgets::default());
        assert_eq!(packed[0].text, "the mill stood at the northern end");
    }

    #[test]
    fn a_line_that_comes_back_later_is_kept() {
        let cues = [
            cue(0.0, 1.0, "yes"),
            cue(1.0, 2.0, "no"),
            cue(2.0, 3.0, "yes"),
        ];
        assert_eq!(pack(&cues, &Budgets::default())[0].text, "yes no yes");
    }

    #[test]
    fn an_empty_cue_is_not_a_break_in_the_repetition() {
        let cues = [
            cue(0.0, 1.0, "same"),
            cue(1.0, 2.0, "   "),
            cue(2.0, 3.0, "same"),
        ];
        assert_eq!(pack(&cues, &Budgets::default())[0].text, "same");
    }

    #[test]
    fn a_passage_carries_times_that_can_be_cited() {
        let cues = [cue(12.5, 15.25, "the mill stood")];
        let packed = pack(&cues, &Budgets::default());
        assert_eq!(packed[0].start_time, Some(12.5));
        assert_eq!(packed[0].end_time, Some(15.25));
        assert_eq!(packed[0].page, None);
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(pack(&[], &Budgets::default()).is_empty());
        assert!(pack(&[cue(0.0, 1.0, "  ")], &Budgets::default()).is_empty());
    }
}
