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

#[cfg(feature = "capi")]
pub mod capi;

// Only ever compiled for a wasm32 target. The feature can be on elsewhere —
// `--all-features` turns everything on — and there it compiles to nothing.
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
pub mod wasm;

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

/// Cut a timed transcript into topic chapters.
///
/// `duration` is how long the recording is, in seconds; it decides how many
/// chapters to aim for. Zero or less means take it from the last cue.
pub fn segment_cues(cues: &[Cue], duration: f64) -> Vec<cueseg::Segment> {
    segment_cues_with(cues, duration, None)
}

/// The same, given vectors somebody else computed for the windows.
///
/// Without them the windows are compared by the words they repeat, which
/// cannot tell that "ships" and "vessels" are one subject. [`cue_windows`]
/// gives the text to embed, in the order the vectors must come back in.
pub fn segment_cues_with(
    cues: &[Cue],
    duration: f64,
    embeddings: Option<&[Vec<f64>]>,
) -> Vec<cueseg::Segment> {
    let duration = match duration > 0.0 {
        true => duration,
        false => cues.last().map_or(0.0, |cue| cue.end),
    };
    cueseg::segment_with(cues, duration, embeddings)
}

/// The text of each window a transcript is measured in, to embed.
pub fn cue_windows(cues: &[Cue]) -> Vec<String> {
    cueseg::windows_of(cues)
        .into_iter()
        .map(|window| window.text)
        .collect()
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

/// Read an identifier — a DOI, ISSN, ISBN or arXiv id — out of whatever it
/// arrived wrapped in.
pub fn parse_identifier(text: &str) -> Option<idnorm::Identifier> {
    idnorm::parse(text)
}

/// What a closed subject scheme says about a set of headings.
///
/// The scheme is supplied rather than built in — which subjects a collection
/// uses is the most collection-specific thing there is. These take it as the
/// record `bibclean` reads from a file.
pub fn canonical_subject(scheme: &bibclean::scheme::Scheme, name: &str) -> Option<String> {
    scheme.canonical(name).map(str::to_string)
}

/// The subjects a book should carry: the chosen ones, their parents, and
/// nothing the title rules out.
pub fn compose_subjects(
    scheme: &bibclean::scheme::Scheme,
    subjects: &[String],
    title: Option<&str>,
) -> Vec<String> {
    scheme.compose(subjects, title)
}

/// Whether a book has no heading the scheme recognises.
pub fn needs_subject(scheme: &bibclean::scheme::Scheme, existing: &[String]) -> bool {
    scheme.needs_subject(existing)
}

/// The best author a record's own title and front matter can offer.
///
/// `None` when neither says anything that reads as a person's name, which is
/// the honest answer for a scanned book with no title page.
pub fn suggest_author(
    title: &str,
    front_matter: Option<&str>,
) -> Option<bibclean::author::Suggestion> {
    bibclean::author::best(title, front_matter)
}

/// Whether a chunk of body text carries footnote or endnote citations.
///
/// The structural signal a converter leaves on EPUB3 note definitions first;
/// otherwise the numbered-note markers and what sits after them.
pub fn looks_like_footnote_citations(text: &str) -> bool {
    chunkmd::footnote::looks_like_citations(text)
}

/// Read the references out of a bibliography or a page of notes.
///
/// Each carries `unsettled`: the fields that could not be read off the entry,
/// named so a caller can ask somebody else about exactly those and no more.
pub fn find_citations(text: &str) -> Vec<citefind::Reference> {
    citefind::read(text)
}

/// Compose the question to put to a model about the references that did not
/// parse. `None` when every one of them settled.
pub fn citation_prompt(references: &[citefind::Reference]) -> Option<String> {
    citefind::prompt::compose(references)
}

/// Fill in what did not parse from a model's reply, and return the references
/// with the answer applied. `Err` when the reply is not one JSON object.
pub fn merge_citation_reply(
    references: &[citefind::Reference],
    reply: &str,
) -> Result<Vec<citefind::Reference>, String> {
    let mut references = references.to_vec();
    citefind::prompt::merge(&mut references, reply)?;
    Ok(references)
}

/// The series a title belongs to, and which volume of it this is.
pub fn series_of(title: &str, author: Option<&str>) -> Option<seriesof::Proposal> {
    seriesof::series_of(title, author)
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
    fn the_author_surface_agrees_with_the_library_underneath() {
        const TITLE: &str = "Aquinas (Companion)";
        const FRONT: &str = "THE CAMBRIDGE COMPANION TO AQUINAS\n\nedited by Norman Kretzmann";
        assert_eq!(
            suggest_author(TITLE, Some(FRONT)),
            bibclean::author::best(TITLE, Some(FRONT))
        );
        assert!(suggest_author("", None).is_none());
    }

    #[test]
    fn the_footnote_signal_agrees_with_the_library_underneath() {
        const NOTES: &str = "1. John Smith, *A Title* (Oxford: Clarendon Press, 1998), 183.\n\
                             2. Jane Doe, *Another* (Cambridge: University Press, 2001), p. 12.";
        assert!(looks_like_footnote_citations(NOTES));
        assert_eq!(
            looks_like_footnote_citations(NOTES),
            chunkmd::footnote::looks_like_citations(NOTES)
        );
        assert!(!looks_like_footnote_citations("Just a paragraph of prose."));
    }

    #[test]
    fn the_citation_surface_agrees_with_the_library_underneath() {
        const REFS: &str = "Smith, John. *A Title*. Oxford: Clarendon Press, 1998.\n\nMarron, Kevin Ritual Abuse, Seal Books, 1988";
        let found = find_citations(REFS);
        assert_eq!(found, citefind::read(REFS));

        // The clean entry settles and the comma-only one does not, so there
        // is a question to ask and it is about the second alone.
        let prompt = citation_prompt(&found).expect("something unsettled");
        assert!(!prompt.contains("Clarendon"), "{prompt}");
        assert!(prompt.contains("Ritual Abuse"), "{prompt}");

        let merged = merge_citation_reply(&found, r#"{"1": {"authors": ["Marron, Kevin"]}}"#)
            .expect("a reply");
        assert_eq!(merged[1].authors, ["Marron, Kevin"]);
        assert_eq!(merged[0], found[0]);
        assert!(merge_citation_reply(&found, "not json").is_err());
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
    fn a_transcript_comes_back_as_chapters() {
        // Two halves about two different things, so the words themselves say
        // where the boundary is.
        let cues: Vec<Cue> = (0..1200)
            .map(|i| Cue {
                v: corpus_core::V,
                start: i as f64 * 5.0,
                end: i as f64 * 5.0 + 5.0,
                text: match i < 600 {
                    true => "the mill stood at the northern end of the valley".into(),
                    false => "parliament dissolved itself again amid objections".into(),
                },
            })
            .collect();

        let chapters = segment_cues(&cues, 6000.0);
        assert!(!chapters.is_empty());
        assert_eq!(chapters, cueseg::segment(&cues, 6000.0));
        // The duration can be left to the cues themselves.
        assert_eq!(segment_cues(&cues, 0.0).len(), chapters.len());

        // The windows to embed are the ones the segmenter measures, and
        // vectors given for them are used rather than the words.
        let windows = cue_windows(&cues);
        assert_eq!(windows.len(), cueseg::windows_of(&cues).len());
        let flat: Vec<Vec<f64>> = windows.iter().map(|_| vec![1.0, 0.0]).collect();
        let with_flat = segment_cues_with(&cues, 6000.0, Some(&flat));
        assert_ne!(
            with_flat.len(),
            chapters.len(),
            "vectors saying every window is alike gave the same chapters as the words"
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
    fn an_identifier_comes_back_normalised() {
        let found = parse_identifier("https://doi.org/10.1000/182").expect("an identifier");
        assert_eq!(found.id, "10.1000/182");
        assert_eq!(parse_identifier("hello"), None);
    }

    #[test]
    fn a_series_comes_back_the_same_as_the_library_reads_it() {
        let found = series_of("The Philokalia Vol 3", None).expect("a series");
        assert_eq!(found.series, "The Philokalia");
        assert_eq!(found.volume, Some(3));
        assert_eq!(series_of("Being and Time", None), None);
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
