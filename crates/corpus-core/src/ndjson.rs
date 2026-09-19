//! Newline-delimited JSON, read lazily and written flushed.
//!
//! Every list-shaped stream between these tools is NDJSON: one record per
//! line, produced and consumed one at a time, so a slow stage never has to
//! hold the whole list and never blocks the pipe behind it.

use std::io::{BufRead, Write};

use serde::{Serialize, de::DeserializeOwned};

/// Something went wrong reading or writing a stream.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A line was not the record the reader expected.
    #[error("line {line}: {source}")]
    Parse {
        /// One-based line number within the stream, counting blank lines.
        line: u64,
        /// What serde made of it.
        source: serde_json::Error,
    },
    /// A record could not be turned into JSON at all.
    #[error("encoding a record: {0}")]
    Encode(#[source] serde_json::Error),
    /// The stream itself failed.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// Read a stream of records, one per line.
///
/// Blank lines are skipped, so a stream that was concatenated or hand-edited
/// still reads. The iterator stops at the first error rather than skipping
/// it: a malformed line means the producer and the consumer disagree about
/// the contract, and continuing would bury that.
pub fn read<T, R>(reader: R) -> impl Iterator<Item = Result<T, Error>>
where
    T: DeserializeOwned,
    R: BufRead,
{
    let mut line_no = 0u64;
    reader.lines().filter_map(move |line| {
        line_no += 1;
        match line {
            Err(e) => Some(Err(Error::Io(e))),
            Ok(line) if line.trim().is_empty() => None,
            Ok(line) => Some(serde_json::from_str(&line).map_err(|source| Error::Parse {
                line: line_no,
                source,
            })),
        }
    })
}

/// Write one record and flush it.
///
/// The flush is the point: a consumer downstream should see a record as soon
/// as it exists, not when the producer's buffer happens to fill.
pub fn write_line<T, W>(writer: &mut W, record: &T) -> Result<(), Error>
where
    T: Serialize,
    W: Write,
{
    serde_json::to_writer(&mut *writer, record).map_err(Error::Encode)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Channel, Hit};

    #[test]
    fn reads_a_record_per_line_and_skips_blanks() {
        let input = "{\"id\":\"a\",\"channel\":\"fts\",\"rank\":1,\"score\":1.0}\n\n\
                     {\"id\":\"b\",\"channel\":\"vec\",\"rank\":2,\"score\":0.5}\n";
        let hits: Vec<Hit> = read(input.as_bytes()).collect::<Result<_, _>>().unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[1].id, "b");
        assert_eq!(hits[1].channel, Channel::Vector);
    }

    #[test]
    fn a_bad_line_reports_its_line_number_and_stops_the_stream() {
        let input = "{\"id\":\"a\",\"channel\":\"fts\",\"rank\":1,\"score\":1.0}\nnot json\n";
        let mut stream = read::<Hit, _>(input.as_bytes());
        assert!(stream.next().unwrap().is_ok());
        match stream.next().unwrap() {
            Err(Error::Parse { line, .. }) => assert_eq!(line, 2),
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn writes_one_compact_line_per_record() {
        let mut out = Vec::new();
        write_line(&mut out, &Hit::new("a", Channel::Meta, 1, 0.25)).unwrap();
        write_line(&mut out, &Hit::new("b", Channel::Meta, 2, 0.5)).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.ends_with('\n'));
        assert!(!text.contains("\n  "));
    }

    #[test]
    fn what_is_written_reads_back_identical() {
        let hit = Hit::new("a", Channel::Vector, 3, 0.125);
        let mut out = Vec::new();
        write_line(&mut out, &hit).unwrap();
        let back: Vec<Hit> = read(out.as_slice()).collect::<Result<_, _>>().unwrap();
        assert_eq!(back, vec![hit]);
    }
}
