//! Serde plumbing for floats that JSON can actually carry.
//!
//! Use it as `#[serde(with = "corpus_core::finite")]` on an `f64`, or
//! `#[serde(default, with = "corpus_core::finite::option")]` on an
//! `Option<f64>`.
//!
//! JSON has no spelling for NaN or infinity, and `serde_json` writes them as
//! `null`. That is a silent corruption in both directions: a score written as
//! `null` fails to read back and kills the stream at the consumer, and a
//! timestamp written as `null` reads back as "no timestamp" with nothing to
//! say it was ever anything else. So a non-finite number is refused where it
//! is produced, which is the only place anyone can still fix it.

use serde::{Deserialize, Deserializer, Serializer, de::Error as _, ser::Error as _};

/// Write a float, refusing one JSON cannot carry.
pub fn serialize<S: Serializer>(value: &f64, out: S) -> Result<S::Ok, S::Error> {
    if !value.is_finite() {
        return Err(S::Error::custom(format!(
            "{value} cannot be written as JSON"
        )));
    }
    out.serialize_f64(*value)
}

/// Read a float, refusing one that is not finite.
pub fn deserialize<'de, D: Deserializer<'de>>(input: D) -> Result<f64, D::Error> {
    let value = f64::deserialize(input)?;
    if !value.is_finite() {
        return Err(D::Error::custom(format!("{value} is not a finite number")));
    }
    Ok(value)
}

/// The same, for a field that may be absent.
pub mod option {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    /// Write an optional float, refusing one JSON cannot carry.
    pub fn serialize<S: Serializer>(value: &Option<f64>, out: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => super::serialize(value, out),
            None => out.serialize_none(),
        }
    }

    /// Read an optional float, refusing one that is not finite.
    pub fn deserialize<'de, D: Deserializer<'de>>(input: D) -> Result<Option<f64>, D::Error> {
        let value = Option::<f64>::deserialize(input)?;
        match value {
            Some(value) if !value.is_finite() => {
                Err(D::Error::custom(format!("{value} is not a finite number")))
            }
            value => Ok(value),
        }
    }
}
