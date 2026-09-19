//! One scored candidate, on its way into a fusion.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A single result from a single retrieval channel.
///
/// `rank` is the candidate's position within its own channel, from one, and
/// is what rank fusion consumes; `score` is the channel's raw number, carried
/// along because it is useful to a human reading a trace and useless for
/// comparing across channels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hit {
    /// Record version.
    #[serde(default = "crate::v")]
    pub v: u32,
    /// Identifier of the thing retrieved, opaque to fusion.
    pub id: String,
    /// Which channel produced this hit.
    pub channel: Channel,
    /// Position within the channel's own ranking, from one.
    pub rank: u32,
    /// The channel's raw score, on whatever scale that channel uses.
    #[serde(
        serialize_with = "crate::float::serialize",
        deserialize_with = "crate::float::deserialize"
    )]
    pub score: f64,
    /// Anything the producer wants to carry through fusion untouched.
    ///
    /// Keys serialise in sorted order, which is what makes a fused stream
    /// byte-reproducible. That holds because `serde_json`'s map is a
    /// `BTreeMap`; enabling its `preserve_order` feature anywhere in the
    /// dependency graph would switch it to insertion order and break it. The
    /// test below is what would catch that.
    #[serde(default)]
    pub payload: Map<String, Value>,
}

impl Hit {
    /// A hit with an empty payload.
    pub fn new(id: impl Into<String>, channel: Channel, rank: u32, score: f64) -> Self {
        Self {
            v: crate::V,
            id: id.into(),
            channel,
            rank,
            score,
            payload: Map::new(),
        }
    }
}

impl crate::Record for Hit {
    fn version(&self) -> u32 {
        self.v
    }
}

/// A retrieval channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Channel {
    /// Nearest neighbours in an embedding space.
    #[serde(rename = "vec")]
    Vector,
    /// Full-text search.
    Fts,
    /// A match on metadata — title, author, tag.
    Meta,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_spellings_are_the_wire_spellings() {
        for (channel, wire) in [
            (Channel::Vector, "\"vec\""),
            (Channel::Fts, "\"fts\""),
            (Channel::Meta, "\"meta\""),
        ] {
            assert_eq!(serde_json::to_string(&channel).unwrap(), wire);
            assert_eq!(serde_json::from_str::<Channel>(wire).unwrap(), channel);
        }
    }

    #[test]
    fn every_field_is_emitted_even_when_empty() {
        let hit = Hit::new("abc", Channel::Fts, 1, 0.5);
        assert_eq!(
            serde_json::to_string(&hit).unwrap(),
            r#"{"v":1,"id":"abc","channel":"fts","rank":1,"score":0.5,"payload":{}}"#
        );
    }

    #[test]
    fn a_score_that_is_not_a_number_is_refused_rather_than_written_as_null() {
        let mut hit = Hit::new("a", Channel::Fts, 1, 0.0);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            hit.score = bad;
            let e = serde_json::to_string(&hit).unwrap_err();
            assert!(e.to_string().contains("cannot be written"), "{bad}: {e}");
        }
    }

    #[test]
    fn a_score_that_overflows_a_float_is_refused_on_the_way_in() {
        let line = r#"{"id":"a","channel":"fts","rank":1,"score":1e999}"#;
        assert!(serde_json::from_str::<Hit>(line).is_err());
    }

    #[test]
    fn payload_keys_serialise_in_a_fixed_order() {
        let mut hit = Hit::new("abc", Channel::Vector, 2, 0.25);
        hit.payload.insert("zulu".into(), Value::from(1));
        hit.payload.insert("alpha".into(), Value::from(2));
        assert_eq!(
            serde_json::to_string(&hit).unwrap(),
            r#"{"v":1,"id":"abc","channel":"vec","rank":2,"score":0.25,"payload":{"alpha":2,"zulu":1}}"#
        );
    }
}
