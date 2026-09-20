//! JavaScript and TypeScript, through WebAssembly.
//!
//! # Shape
//!
//! Records come back as **plain objects**, not handles: the same documents
//! the command line writes, with the same field names and the same stability
//! promise. Nothing has to be freed, `JSON.stringify` works, and an array of
//! chunks spreads into state without ceremony.
//!
//! # What is not here
//!
//! The tools that read files or call another program. A browser has no
//! `pdftoppm` and no files, and a binding that pretends otherwise is one that
//! fails at run time rather than at compile time.

use wasm_bindgen::prelude::*;

use crate::{ChunkBudgets, CueBudgets};

/// Hand-written because the generated `.d.ts` would otherwise say `any`: a
/// binding that types its whole return value as `any` is a binding
/// TypeScript cannot check.
#[wasm_bindgen(typescript_custom_section)]
const TYPES: &'static str = r#"
/** A retrievable passage. The same record `chunkmd` writes. */
export interface Chunk {
  v: number;
  /** Position in the stream, from zero. */
  idx: number;
  text: string;
  /** Tokens, by the encoding the budgets count in. */
  tokens: number;
  /** The page it came from, where the source had page markers. */
  page: number | null;
  /** The heading it sits under. */
  chapter: string | null;
  /** "body", "front", "back", "index" or "bibliography". */
  role: string;
  /** Seconds into the recording, for passages made from cues. */
  start_time: number | null;
  end_time: number | null;
  flags: string[];
}

/** One line of a transcript. */
export interface Cue {
  v: number;
  start: number;
  end: number;
  text: string;
}

/** One topic chapter of a recording. */
export interface Segment {
  v: number;
  /** Seconds from the start of the recording. */
  start: number;
  end: number;
  /** A title taken from the chapter's own first sentence. */
  title: string;
  /** The opening of the chapter, for a person choosing between them. */
  excerpt: string;
}

/** One work inside a collection. */
export interface Section {
  v: number;
  idx: number;
  title: string;
  slug: string;
  text: string;
}

/** One scored candidate, before or after fusion. */
export interface Hit {
  v: number;
  id: string;
  /** "vec", "fts" or "meta". */
  channel: string;
  rank: number;
  score: number;
  payload: unknown;
}

/** A DOI, ISSN, ISBN or arXiv identifier, read and checked. */
export interface Identifier {
  v: number;
  /** The text it was read from. */
  input: string;
  /** "doi", "issn", "isbn" or "arxiv". */
  kind: string;
  /** The identifier, in the form it should be stored and compared in. */
  id: string;
  /** Whether the check digit agrees, or null where the kind carries none. */
  checksum: boolean | null;
  /** A URL that resolves it. */
  url?: string;
}

/** One reference out of a bibliography or a page of notes. */
export interface Reference {
  v: number;
  idx: number;
  /** The entry as it appeared, whitespace collapsed. */
  text: string;
  /** The note number it carried, where it carried one. */
  marker?: string;
  /** The names, in the order and the form the entry gave them. */
  authors: string[];
  year?: number;
  /** The work cited — for an article the article, not the journal. */
  title?: string;
  /** The journal it appeared in, or the publisher of a book. */
  container?: string;
  pages?: string;
  ids: { kind: string; id: string }[];
  /** "book", "article", "chapter", "web" or "other". */
  kind: string;
  /** Whether it only points back at an earlier entry. */
  continuation: boolean;
  /** The fields that could not be read off the entry. */
  unsettled: string[];
}

/** What a title says about its series. */
export interface SeriesProposal {
  v: number;
  title: string;
  author?: string;
  series: string;
  /** Which volume, where the title said. Absent for membership without order. */
  volume?: number;
  /** A title reconstructed from publisher shorthand, where one reliably can be. */
  proposed_title?: string;
  /** "volume" or "publisher". */
  method: string;
}

/** What a file is. */
export interface Doc {
  v: number;
  path: string;
  kind: string;
  declared_ext: string | null;
  bytes: number;
  pages: number | null;
  language: string | null;
  text_layer: string | null;
  needs_ocr: boolean | null;
  notes: string[];
}
"#;

/// What the chunker does when nothing is passed.
fn chunk_budgets(
    target: Option<usize>,
    overlap: Option<usize>,
    min_tokens: Option<usize>,
    hard_max: Option<usize>,
) -> ChunkBudgets {
    let default = ChunkBudgets::default();
    ChunkBudgets {
        target: target.unwrap_or(default.target),
        overlap: overlap.unwrap_or(default.overlap),
        min_tokens: min_tokens.unwrap_or(default.min_tokens),
        hard_max: hard_max.unwrap_or(default.hard_max),
    }
}

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsValue::from_str(&e.to_string()))
}

fn from_js<T: serde::de::DeserializeOwned>(value: JsValue, what: &str) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value)
        .map_err(|e| JsValue::from_str(&format!("{what} is not the expected shape: {e}")))
}

/// Cut markdown into retrievable passages.
#[wasm_bindgen(js_name = chunkMarkdown, skip_typescript)]
pub fn chunk_markdown(
    markdown: &str,
    target: Option<usize>,
    overlap: Option<usize>,
    min_tokens: Option<usize>,
    hard_max: Option<usize>,
) -> Result<JsValue, JsValue> {
    let budgets = chunk_budgets(target, overlap, min_tokens, hard_max);
    to_js(&crate::chunk_markdown(markdown, budgets))
}

/// Clean converted markdown.
///
/// `pages` also normalises page markers, which is not idempotent.
#[wasm_bindgen(js_name = scrub)]
pub fn scrub(markdown: &str, pages: Option<bool>) -> String {
    match pages.unwrap_or(false) {
        true => crate::scrub_markdown_pages(markdown),
        false => crate::scrub_markdown(markdown),
    }
}

/// How many tokens a string is.
#[wasm_bindgen(js_name = countTokens)]
pub fn count_tokens(text: &str) -> usize {
    crate::count_tokens(text)
}

/// Read any of the subtitle formats into cues.
#[wasm_bindgen(js_name = parseSubtitles, skip_typescript)]
pub fn parse_subtitles(content: &str, keep_repeats: Option<bool>) -> Result<JsValue, JsValue> {
    let cues = crate::parse_subtitles(content);
    let cues = match keep_repeats.unwrap_or(false) {
        true => cues,
        false => crate::merge_repeats(&cues),
    };
    to_js(&cues)
}

/// Pack cues into passages worth embedding.
#[wasm_bindgen(js_name = packCues, skip_typescript)]
pub fn pack_cues(
    cues: JsValue,
    seconds: Option<f64>,
    max_tokens: Option<usize>,
) -> Result<JsValue, JsValue> {
    let default = CueBudgets::default();
    let budgets = CueBudgets {
        seconds: seconds.unwrap_or(default.seconds),
        max_tokens: max_tokens.unwrap_or(default.max_tokens),
    };
    let cues: Vec<cuekit::Cue> = from_js(cues, "cues")?;
    to_js(&crate::pack_cues(&cues, budgets))
}

/// Cut a timed transcript into topic chapters.
///
/// `duration` is the recording's length in seconds; omit it and the last cue
/// decides.
/// `embeddings` is one vector per window, in the order `cueWindows` gives
/// them; without it the windows are compared by the words they repeat.
#[wasm_bindgen(js_name = segmentCues, skip_typescript)]
pub fn segment_cues(
    cues: JsValue,
    duration: Option<f64>,
    embeddings: JsValue,
) -> Result<JsValue, JsValue> {
    let cues: Vec<cuekit::Cue> = from_js(cues, "cues")?;
    let vectors: Option<Vec<Vec<f64>>> = match embeddings.is_undefined() || embeddings.is_null() {
        true => None,
        false => Some(from_js(embeddings, "embeddings")?),
    };
    to_js(&crate::segment_cues_with(
        &cues,
        duration.unwrap_or(0.0),
        vectors.as_deref(),
    ))
}

/// The text of each window a transcript is measured in, to embed.
#[wasm_bindgen(js_name = cueWindows, skip_typescript)]
pub fn cue_windows(cues: JsValue) -> Result<JsValue, JsValue> {
    let cues: Vec<cuekit::Cue> = from_js(cues, "cues")?;
    to_js(&crate::cue_windows(&cues))
}

/// Split a collection into the works inside it.
///
/// Throws when the document is one work rather than a collection, which is
/// the usual answer, with the reason each candidate was rejected.
#[wasm_bindgen(js_name = splitCollection, skip_typescript)]
pub fn split_collection(markdown: &str) -> Result<JsValue, JsValue> {
    match crate::split_collection(markdown) {
        Ok(sections) => to_js(&sections),
        Err(why) => Err(JsValue::from_str(&why)),
    }
}

/// Fuse ranked lists into one.
#[wasm_bindgen(js_name = fuseHits, skip_typescript)]
pub fn fuse_hits(
    hits: JsValue,
    k: Option<f64>,
    weights: JsValue,
    limit: Option<usize>,
) -> Result<JsValue, JsValue> {
    let hits: Vec<corpus_core::Hit> = from_js(hits, "hits")?;
    let weights: Vec<(String, f64)> = match weights.is_undefined() || weights.is_null() {
        true => Vec::new(),
        false => from_js::<std::collections::HashMap<String, f64>>(weights, "weights")?
            .into_iter()
            .collect(),
    };
    to_js(&crate::fuse_hits(
        &hits,
        k.unwrap_or(rrf::K),
        &weights,
        limit,
    ))
}

/// Say what a file is from its first bytes and its name.
#[wasm_bindgen(js_name = identify, skip_typescript)]
pub fn identify(name: &str, data: &[u8]) -> Result<JsValue, JsValue> {
    to_js(&crate::identify(name, data))
}

/// Read the references out of a bibliography or a page of notes.
#[wasm_bindgen(js_name = findCitations, skip_typescript)]
pub fn find_citations(text: &str) -> Result<JsValue, JsValue> {
    to_js(&crate::find_citations(text))
}

/// The question to put to a model about the references that did not parse.
///
/// `undefined` when every one of them settled.
#[wasm_bindgen(js_name = citationPrompt, skip_typescript)]
pub fn citation_prompt(references: JsValue) -> Result<JsValue, JsValue> {
    let references: Vec<citefind::Reference> = from_js(references, "references")?;
    match crate::citation_prompt(&references) {
        Some(prompt) => to_js(&prompt),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Fill in what did not parse from a model's reply.
#[wasm_bindgen(js_name = mergeCitationReply, skip_typescript)]
pub fn merge_citation_reply(references: JsValue, reply: &str) -> Result<JsValue, JsValue> {
    let references: Vec<citefind::Reference> = from_js(references, "references")?;
    let merged = crate::merge_citation_reply(&references, reply).map_err(JsValue::from)?;
    to_js(&merged)
}

/// The series a title belongs to, and which volume of it this is.
///
/// `undefined` when the title says nothing about a series.
#[wasm_bindgen(js_name = seriesOf, skip_typescript)]
pub fn series_of(title: &str, author: Option<String>) -> Result<JsValue, JsValue> {
    match crate::series_of(title, author.as_deref()) {
        Some(found) => to_js(&found),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Read an identifier out of whatever it arrived wrapped in.
///
/// `undefined` when the string is not one.
#[wasm_bindgen(js_name = parseIdentifier, skip_typescript)]
pub fn parse_identifier(text: &str) -> Result<JsValue, JsValue> {
    match crate::parse_identifier(text) {
        Some(found) => to_js(&found),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Declarations for the functions whose return type the generator cannot
/// infer, so a caller gets `Chunk[]` rather than `any`.
#[wasm_bindgen(typescript_custom_section)]
const SIGNATURES: &'static str = r#"
export function chunkMarkdown(markdown: string, target?: number, overlap?: number, min_tokens?: number, hard_max?: number): Chunk[];
export function parseSubtitles(content: string, keep_repeats?: boolean): Cue[];
export function packCues(cues: Cue[], seconds?: number, max_tokens?: number): Chunk[];
export function segmentCues(cues: Cue[], duration?: number, embeddings?: number[][]): Segment[];
export function cueWindows(cues: Cue[]): string[];
export function splitCollection(markdown: string): Section[];
export function fuseHits(hits: Hit[], k?: number, weights?: Record<string, number>, limit?: number): Hit[];
export function identify(name: string, data: Uint8Array): Doc;
export function findCitations(text: string): Reference[];
export function citationPrompt(references: Reference[]): string | undefined;
export function mergeCitationReply(references: Reference[], reply: string): Reference[];
export function seriesOf(title: string, author?: string): SeriesProposal | undefined;
export function parseIdentifier(text: string): Identifier | undefined;
"#;
