//! Record types and stream plumbing shared by every catena tool.
//!
//! Three records travel between the tools: a [`Doc`] is what a file is, a
//! [`Chunk`] is a retrievable passage with enough provenance to cite it, and a
//! [`Hit`] is one scored candidate on its way into a fusion. Each carries a
//! version field so a reader can refuse a stream it does not understand.
//!
//! Every list-shaped stream is NDJSON — see [`ndjson`] — and every predicate
//! answers through an exit code — see [`exit`].

pub mod chunk;
pub mod doc;
pub mod exit;
pub mod finite;
pub mod hit;
pub mod ndjson;

pub use chunk::{Chunk, Role};
pub use doc::{Doc, Kind, TextLayer};
pub use hit::{Channel, Hit};
pub use ndjson::{Error, read, write_line};

/// The record version these types serialise as.
pub const V: u32 = 1;

/// A record that travels between tools as one line of NDJSON.
///
/// The version is on the trait rather than read off the struct so that
/// [`ndjson::read`] can refuse a stream from a future version of the suite
/// instead of parsing it into whatever happens to line up.
pub trait Record: serde::Serialize + serde::de::DeserializeOwned {
    /// The version this record announced.
    fn version(&self) -> u32;
}

/// Serde default for the `v` field, so a record written by an older tool that
/// omitted it still reads back as version 1 rather than 0.
pub(crate) const fn v() -> u32 {
    V
}
