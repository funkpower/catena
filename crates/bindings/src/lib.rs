//! The catena libraries behind one surface, for calling in process.
//!
//! Every tool in this workspace is a library with a thin command on top, and
//! a pipeline written in another language can have either. A subprocess is
//! the right answer most of the time: it is a process boundary, it composes
//! with a shell, and it costs a few milliseconds. In process is the right
//! answer when the caller is already holding the text — a stage that would
//! otherwise write a file so that it can read it straight back.
//!
//! What is bound here is the **record shapes**, not the internals. Those
//! shapes are documented, versioned, and what every tool already writes to
//! stdout; binding the structs behind them would publish field names that are
//! free to move and leave the two forms disagreeing about the same chunk.
//!
//! Nothing here grows logic of its own. If a binding needs something the
//! libraries cannot do, that is a missing library function.

#[cfg(feature = "python")]
mod python;

use corpus_core::{Chunk, Doc, Hit};
use cuekit::Cue;

/// Where chunks are flushed, and how large they may get.
#[derive(Debug, Clone, Copy)]
pub struct ChunkBudgets {
    /// Flush a chunk at this many tokens.
    pub target: usize,
    /// Carry this much of each chunk into the next one.
    pub overlap: usize,
    /// Drop chunks smaller than this.
    pub min_tokens: usize,
    /// Never emit a chunk larger than this.
    pub hard_max: usize,
}

impl Default for ChunkBudgets {
    fn default() -> Self {
        let budgets = chunkmd::Budgets::default();
        Self {
            target: budgets.target,
            overlap: budgets.overlap,
            min_tokens: budgets.min_tokens,
            hard_max: budgets.hard_max,
        }
    }
}

impl From<ChunkBudgets> for chunkmd::Budgets {
    fn from(budgets: ChunkBudgets) -> Self {
        chunkmd::Budgets {
            target: budgets.target,
            overlap: budgets.overlap,
            min_tokens: budgets.min_tokens,
            hard_max: budgets.hard_max,
        }
    }
}

/// How much of a recording a passage covers.
#[derive(Debug, Clone, Copy)]
pub struct CueBudgets {
    /// Seconds of audio a passage covers.
    pub seconds: f64,
    /// Tokens a passage may hold, whatever the clock says.
    pub max_tokens: usize,
}

impl Default for CueBudgets {
    fn default() -> Self {
        let budgets = cuepack::Budgets::default();
        Self {
            seconds: budgets.seconds,
            max_tokens: budgets.max_tokens,
        }
    }
}

/// Cut markdown into retrievable passages.
pub fn chunk_markdown(markdown: &str, budgets: ChunkBudgets) -> Vec<Chunk> {
    chunkmd::chunk(markdown, &budgets.into())
}

/// Clean converted markdown.
pub fn scrub_markdown(markdown: &str) -> String {
    mdscrub::scrub(markdown)
}

/// Clean converted markdown, normalising page markers as well.
///
/// Separate because it is not idempotent: running it twice renumbers the
/// pages it moved the first time.
pub fn scrub_markdown_pages(markdown: &str) -> String {
    mdscrub::pages::normalize(&mdscrub::scrub(markdown))
}

/// How many tokens a string is, by the encoding every budget here counts in.
pub fn count_tokens(text: &str) -> usize {
    textkit::count(text)
}

/// Read any of the subtitle formats into cues.
pub fn parse_subtitles(content: &str) -> Vec<Cue> {
    cuekit::parse(content)
}

/// Collapse the runs of repeated cues a live caption track leaves behind.
pub fn merge_repeats(cues: &[Cue]) -> Vec<Cue> {
    cuekit::merge_repeats(cues)
}

/// Pack cues into passages worth embedding.
pub fn pack_cues(cues: &[Cue], budgets: CueBudgets) -> Vec<Chunk> {
    cuepack::pack(
        cues,
        &cuepack::Budgets {
            seconds: budgets.seconds,
            max_tokens: budgets.max_tokens,
        },
    )
}

/// Split a collection into the works inside it.
///
/// `Err` when the document is one work rather than a collection, which is the
/// usual answer and not a failure. The message says what each candidate
/// produced.
pub fn split_collection(markdown: &str) -> Result<Vec<mdsplit::Section>, String> {
    mdsplit::split(markdown).map_err(|why| why.to_string())
}

/// Fuse ranked lists into one.
///
/// `weights` is read as channel name to weight; a channel not named is worth
/// one. An unknown name is ignored rather than refused, because a caller
/// fusing two channels should not have to know about a third.
pub fn fuse_hits(
    hits: &[Hit],
    k: f64,
    weights: &[(String, f64)],
    limit: Option<usize>,
) -> Vec<Hit> {
    let mut channels = std::collections::HashMap::new();
    for (name, weight) in weights {
        if let Some(channel) = channel_named(name) {
            channels.insert(channel, *weight);
        }
    }
    let fused = rrf::fuse(hits, &rrf::Weights { k, channels });
    match limit {
        Some(limit) => fused.into_iter().take(limit).collect(),
        None => fused,
    }
}

/// The channel a name refers to, spelled as the records spell it.
fn channel_named(name: &str) -> Option<corpus_core::Channel> {
    match name {
        "vec" => Some(corpus_core::Channel::Vector),
        "fts" => Some(corpus_core::Channel::Fts),
        "meta" => Some(corpus_core::Channel::Meta),
        _ => None,
    }
}

/// Say what a file is from its bytes and its name.
pub fn identify(name: &str, bytes: &[u8]) -> Doc {
    booksniff::sniff(&bytes, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKDOWN: &str = "# Chapter One\n\n<!-- Page 41 -->\n\nThe obligation ran both ways, and contemporaries said so plainly whenever they were asked. A lord who could not protect was a lord who could not command.\n";

    #[test]
    fn the_surface_agrees_with_the_library_underneath() {
        assert_eq!(
            chunk_markdown(MARKDOWN, ChunkBudgets::default()),
            chunkmd::chunk(MARKDOWN, &chunkmd::Budgets::default())
        );
        assert_eq!(scrub_markdown(MARKDOWN), mdscrub::scrub(MARKDOWN));
        assert_eq!(count_tokens("a passage"), textkit::count("a passage"));
    }

    #[test]
    fn the_default_budgets_are_the_tools_defaults() {
        let ours = ChunkBudgets::default();
        let theirs = chunkmd::Budgets::default();
        assert_eq!(
            (ours.target, ours.overlap, ours.min_tokens, ours.hard_max),
            (
                theirs.target,
                theirs.overlap,
                theirs.min_tokens,
                theirs.hard_max
            )
        );
        assert_eq!(
            CueBudgets::default().seconds,
            cuepack::Budgets::default().seconds
        );
    }

    #[test]
    fn a_document_that_is_one_work_says_so_rather_than_splitting() {
        assert!(split_collection(MARKDOWN).is_err());
    }

    #[test]
    fn subtitles_come_back_as_cues_and_pack_into_passages() {
        let srt = "1\n00:00:01,000 --> 00:00:04,000\nthe mill stood at the northern end\n\n";
        let cues = parse_subtitles(srt);
        assert_eq!(cues.len(), 1);
        let packed = pack_cues(&cues, CueBudgets::default());
        assert_eq!(packed.len(), 1);
        assert_eq!(packed[0].start_time, Some(1.0));
    }

    #[test]
    fn a_channel_nobody_here_knows_is_ignored_rather_than_refused() {
        assert_eq!(channel_named("vec"), Some(corpus_core::Channel::Vector));
        assert_eq!(channel_named("lexical"), None);
    }

    #[test]
    fn bytes_are_identified_the_same_way_a_file_is() {
        let pdf = b"%PDF-1.4\n";
        assert_eq!(
            identify("book.pdf", pdf).kind,
            booksniff::sniff(&&pdf[..], "book.pdf").kind
        );
    }
}
