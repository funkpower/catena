//! A retrievable passage, with enough provenance to cite it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One passage of a document or a recording.
///
/// Documents fill `page` and `chapter`; media fill `start_time` and
/// `end_time`. The shape is the same either way, so one index serves a book
/// and a lecture recording and can cite both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Chunk {
    /// Record version.
    #[serde(default = "crate::v")]
    pub v: u32,
    /// Position in the stream this chunk was cut from, from zero.
    pub idx: u32,
    /// The passage.
    pub text: String,
    /// Token count of `text`, by the tokeniser the chunker used.
    pub tokens: u32,
    /// Page the passage starts on, for paginated sources.
    pub page: Option<u32>,
    /// Nearest enclosing heading, as written.
    pub chapter: Option<String>,
    /// What part of the work the passage belongs to.
    pub role: Role,
    /// Seconds from the start of the recording, for timed sources.
    #[serde(default, with = "crate::finite::option")]
    #[schemars(with = "Option<f64>")]
    pub start_time: Option<f64>,
    /// Seconds from the start of the recording, for timed sources.
    #[serde(default, with = "crate::finite::option")]
    #[schemars(with = "Option<f64>")]
    pub end_time: Option<f64>,
    /// Set tags; see [`flags`]. Sorted and deduplicated on the way in and on
    /// the way out, so the same set of flags is always the same bytes
    /// whatever order the producer set them in.
    #[serde(default, deserialize_with = "sorted_flags")]
    pub flags: Vec<String>,
}

impl Chunk {
    /// A body chunk with no provenance yet.
    pub fn new(idx: u32, text: impl Into<String>, tokens: u32) -> Self {
        Self {
            v: crate::V,
            idx,
            text: text.into(),
            tokens,
            page: None,
            chapter: None,
            role: Role::Body,
            start_time: None,
            end_time: None,
            flags: Vec::new(),
        }
    }

    /// Set a flag. Flags stay sorted and unique so the same input always
    /// serialises to the same bytes.
    pub fn flag(&mut self, flag: impl Into<String>) {
        let flag = flag.into();
        if let Err(at) = self.flags.binary_search(&flag) {
            self.flags.insert(at, flag);
        }
    }

    /// Whether a flag is set.
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|set| set == flag)
    }
}

impl crate::Record for Chunk {
    fn version(&self) -> u32 {
        self.v
    }
}

/// Sort and deduplicate flags as they are read, so a record written by
/// another implementation gets the same treatment as one built here.
fn sorted_flags<'de, D: serde::Deserializer<'de>>(input: D) -> Result<Vec<String>, D::Error> {
    let mut flags = Vec::<String>::deserialize(input)?;
    flags.sort_unstable();
    flags.dedup();
    Ok(flags)
}

/// The flag names the tools in this workspace set.
pub mod flags {
    /// The passage's footnote markers were resolved against a citation list.
    pub const FOOTNOTE_CITATIONS: &str = "footnote_citations";
}

/// What part of a work a passage belongs to.
///
/// Retrieval wants body text; an index or a bibliography matches lexically
/// far too well for what it is worth, which is why the distinction is carried
/// on the record rather than rediscovered at query time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Running text.
    #[default]
    Body,
    /// A works-cited or references list.
    Bibliography,
    /// A back-of-book index.
    Index,
    /// A table of contents.
    Toc,
    /// Title page, copyright page, dedication, and the rest of the front.
    Frontmatter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_spellings_are_the_wire_spellings() {
        for (role, wire) in [
            (Role::Body, "\"body\""),
            (Role::Bibliography, "\"bibliography\""),
            (Role::Index, "\"index\""),
            (Role::Toc, "\"toc\""),
            (Role::Frontmatter, "\"frontmatter\""),
        ] {
            assert_eq!(serde_json::to_string(&role).unwrap(), wire);
            assert_eq!(serde_json::from_str::<Role>(wire).unwrap(), role);
        }
    }

    #[test]
    fn every_field_is_emitted_even_when_empty() {
        let chunk = Chunk::new(0, "hello", 1);
        assert_eq!(
            serde_json::to_string(&chunk).unwrap(),
            r#"{"v":1,"idx":0,"text":"hello","tokens":1,"page":null,"chapter":null,"role":"body","start_time":null,"end_time":null,"flags":[]}"#
        );
    }

    #[test]
    fn flags_read_from_a_stream_are_sorted_and_unique_too() {
        let chunk: Chunk = serde_json::from_str(
            r#"{"idx":0,"text":"x","tokens":1,"role":"body","flags":["zebra","alpha","alpha"]}"#,
        )
        .unwrap();
        assert_eq!(chunk.flags, ["alpha", "zebra"]);
        assert!(chunk.has_flag("zebra"));
        assert!(chunk.has_flag("alpha"));
    }

    #[test]
    fn a_timestamp_that_is_not_a_number_is_refused_rather_than_dropped() {
        let mut chunk = Chunk::new(0, "x", 1);
        chunk.start_time = Some(f64::NAN);
        let e = serde_json::to_string(&chunk).unwrap_err();
        assert!(e.to_string().contains("cannot be written"), "{e}");
    }

    #[test]
    fn flags_stay_sorted_and_unique() {
        let mut chunk = Chunk::new(0, "x", 1);
        chunk.flag("zebra");
        chunk.flag(flags::FOOTNOTE_CITATIONS);
        chunk.flag("zebra");
        assert_eq!(chunk.flags, ["footnote_citations", "zebra"]);
        assert!(chunk.has_flag("zebra"));
        assert!(!chunk.has_flag("aardvark"));
    }
}
