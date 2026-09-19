//! Tokenising and segmenting text, the way the catena tools agree to do it.
//!
//! Two jobs, both of which have to give the same answer everywhere or chunk
//! boundaries move: counting tokens the way the embedding stack counts them,
//! and cutting text into sentences and paragraphs the way the existing
//! pipelines cut it.
//!
//! The segmenters reproduce Python's `re` semantics deliberately, down to
//! which characters count as whitespace, because their output decides where a
//! chunk ends and a chunk boundary that moves is a chunk that no longer
//! matches what is already in an index.

pub mod number;
pub mod pattern;
pub mod segment;
pub mod tokens;

pub use number::{decimal, percent, round_half_to_even};
pub use segment::{is_space, paragraphs, sentences};
pub use tokens::{count, decode_lossy, encode, split_at_token_limit};
