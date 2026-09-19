//! Newline-delimited JSON, read lazily and written flushed.
//!
//! Every list-shaped stream between these tools is NDJSON: one record per
//! line, produced and consumed one at a time, so a slow stage never has to
//! hold the whole list and never blocks the pipe behind it.
//!
//! The reader is deliberately strict. It refuses a record whose version it
//! does not know, it stops at the first bad line rather than skipping past a
//! disagreement about the contract, and it bounds how much one line may cost.
//! A filter pointed at a binary file should fail, not allocate the file.

use std::io::{BufRead, ErrorKind, Read, Write};

use serde::Serialize;

use crate::Record;

/// How long one line may be before the reader gives up, in bytes.
///
/// Generous for a record — a chunk of a thousand tokens is a few kilobytes —
/// and small enough that a file with no newlines in it cannot be read into
/// memory by accident.
pub const MAX_LINE: usize = 16 * 1024 * 1024;

/// A UTF-8 byte-order mark, which Windows tooling puts at the front of files
/// and which is not whitespace, not JSON, and not the producer's fault.
const BOM: &[u8] = &[0xef, 0xbb, 0xbf];

/// Something went wrong reading or writing a stream.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A line was not the record the reader expected.
    #[error("stream line {line}: {source}")]
    Parse {
        /// One-based line number within the stream, counting blank lines.
        line: u64,
        /// What serde made of it.
        source: serde_json::Error,
    },
    /// A line was not UTF-8 at all.
    #[error("stream line {line}: not valid UTF-8")]
    Encoding {
        /// One-based line number within the stream.
        line: u64,
    },
    /// A record announced a version this build does not implement.
    #[error("stream line {line}: record version {found}, expected {expected}")]
    Version {
        /// One-based line number within the stream.
        line: u64,
        /// The version the record announced.
        found: u32,
        /// The version this build understands.
        expected: u32,
    },
    /// A line exceeded [`MAX_LINE`].
    #[error("stream line {line}: longer than the {limit} byte line limit")]
    TooLong {
        /// One-based line number within the stream.
        line: u64,
        /// The limit that was exceeded.
        limit: usize,
    },
    /// A record could not be turned into JSON at all.
    #[error("encoding a record: {0}")]
    Encode(#[source] serde_json::Error),
    /// The stream itself failed.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Whether this is the consumer downstream having closed the pipe.
    ///
    /// Not an error in a filter: `… | head -1` is a normal way to use one.
    /// A tool that sees this should stop and exit successfully, the way every
    /// other program in a pipeline does.
    pub fn is_broken_pipe(&self) -> bool {
        matches!(self, Error::Io(e) if e.kind() == ErrorKind::BrokenPipe)
    }

    /// The line this error happened on, when it happened on one.
    pub fn line(&self) -> Option<u64> {
        match self {
            Error::Parse { line, .. }
            | Error::Encoding { line }
            | Error::Version { line, .. }
            | Error::TooLong { line, .. } => Some(*line),
            Error::Encode(_) | Error::Io(_) => None,
        }
    }
}

/// Read a stream of records, one per line, with the default line limit.
pub fn read<T, R>(reader: R) -> impl Iterator<Item = Result<T, Error>>
where
    T: Record,
    R: BufRead,
{
    read_with_limit(reader, MAX_LINE)
}

/// Read a stream of records, one per line, refusing any line longer than
/// `limit` bytes.
///
/// Blank lines are skipped, so a stream that was concatenated or hand-edited
/// still reads, and a byte-order mark at the very front is ignored. The
/// iterator ends at the first error: a bad line means the producer and the
/// consumer disagree about the contract, and reading on would bury that.
pub fn read_with_limit<T, R>(mut reader: R, limit: usize) -> impl Iterator<Item = Result<T, Error>>
where
    T: Record,
    R: BufRead,
{
    let mut line_no = 0u64;
    let mut buf = Vec::new();
    let mut done = false;
    let mut at_start = true;

    std::iter::from_fn(move || {
        if done {
            return None;
        }
        loop {
            buf.clear();
            // Bounded so that a stream with no newline in it costs `limit`
            // bytes and not the whole stream.
            let read = (&mut reader)
                .take(limit as u64 + 1)
                .read_until(b'\n', &mut buf);
            let read = match read {
                Ok(read) => read,
                Err(e) => {
                    done = true;
                    return Some(Err(Error::Io(e)));
                }
            };
            if read == 0 {
                done = true;
                return None;
            }
            line_no += 1;

            let mut line = buf.as_slice();
            if line.last() == Some(&b'\n') {
                line = &line[..line.len() - 1];
                if line.last() == Some(&b'\r') {
                    line = &line[..line.len() - 1];
                }
            } else if read > limit {
                done = true;
                return Some(Err(Error::TooLong {
                    line: line_no,
                    limit,
                }));
            }
            if at_start {
                at_start = false;
                if let Some(rest) = line.strip_prefix(BOM) {
                    line = rest;
                }
            }

            let line = match std::str::from_utf8(line) {
                Ok(line) => line,
                Err(_) => {
                    done = true;
                    return Some(Err(Error::Encoding { line: line_no }));
                }
            };
            if line.trim().is_empty() {
                continue;
            }

            return Some(match serde_json::from_str::<T>(line) {
                Err(source) => {
                    done = true;
                    Err(Error::Parse {
                        line: line_no,
                        source,
                    })
                }
                Ok(record) if record.version() != crate::V => {
                    done = true;
                    Err(Error::Version {
                        line: line_no,
                        found: record.version(),
                        expected: crate::V,
                    })
                }
                Ok(record) => Ok(record),
            });
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
    // A write failure arrives wrapped in a serde error; unwrapping it here is
    // what lets a caller tell a closed pipe from a record it could not encode.
    serde_json::to_writer(&mut *writer, record).map_err(|e| {
        if e.is_io() {
            Error::Io(e.into())
        } else {
            Error::Encode(e)
        }
    })?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Channel, Chunk, Hit};

    fn hit_line(id: &str) -> String {
        format!("{{\"id\":\"{id}\",\"channel\":\"fts\",\"rank\":1,\"score\":1.0}}")
    }

    #[test]
    fn reads_a_record_per_line_and_skips_blanks() {
        let input = format!("{}\n\n{}\n", hit_line("a"), hit_line("b"));
        let hits: Vec<Hit> = read(input.as_bytes()).collect::<Result<_, _>>().unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[1].id, "b");
    }

    #[test]
    fn a_last_line_without_a_newline_still_reads() {
        let hits: Vec<Hit> = read(hit_line("a").as_bytes())
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn crlf_endings_read() {
        let input = format!("{}\r\n{}\r\n", hit_line("a"), hit_line("b"));
        let hits: Vec<Hit> = read(input.as_bytes()).collect::<Result<_, _>>().unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn a_byte_order_mark_does_not_eat_the_first_record() {
        let input = format!("\u{feff}{}\n", hit_line("a"));
        let hits: Vec<Hit> = read(input.as_bytes()).collect::<Result<_, _>>().unwrap();
        assert_eq!(hits[0].id, "a");
    }

    #[test]
    fn a_bad_line_reports_its_line_number_and_ends_the_stream() {
        let input = format!("\n\n{}\nnot json\n{}\n", hit_line("a"), hit_line("b"));
        let mut stream = read::<Hit, _>(input.as_bytes());
        assert!(stream.next().unwrap().is_ok());
        match stream.next().unwrap() {
            Err(e) => assert_eq!(e.line(), Some(4)),
            other => panic!("expected a parse error, got {other:?}"),
        }
        assert!(
            stream.next().is_none(),
            "reading continued past a broken contract"
        );
    }

    #[test]
    fn a_line_that_is_not_utf8_is_a_data_error_not_an_io_error() {
        let input: &[u8] = b"{\"id\":\"\xff\xfe\"}\n";
        match read::<Hit, _>(input).next().unwrap() {
            Err(Error::Encoding { line }) => assert_eq!(line, 1),
            other => panic!("expected an encoding error, got {other:?}"),
        }
    }

    #[test]
    fn an_endless_line_is_refused_rather_than_buffered() {
        let input = vec![b'x'; 4096];
        match read_with_limit::<Hit, _>(input.as_slice(), 128)
            .next()
            .unwrap()
        {
            Err(Error::TooLong { line, limit }) => {
                assert_eq!(line, 1);
                assert_eq!(limit, 128);
            }
            other => panic!("expected a length error, got {other:?}"),
        }
    }

    #[test]
    fn a_record_from_a_future_version_is_refused() {
        let input = "{\"v\":2,\"id\":\"a\",\"channel\":\"fts\",\"rank\":1,\"score\":1.0}\n";
        match read::<Hit, _>(input.as_bytes()).next().unwrap() {
            Err(Error::Version {
                found, expected, ..
            }) => {
                assert_eq!((found, expected), (2, 1));
            }
            other => panic!("expected a version error, got {other:?}"),
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
    }

    #[test]
    fn a_newline_inside_a_record_stays_one_line() {
        let mut out = Vec::new();
        write_line(&mut out, &Chunk::new(0, "two\nlines", 2)).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 1);
        let back: Vec<Chunk> = read(text.as_bytes()).collect::<Result<_, _>>().unwrap();
        assert_eq!(back[0].text, "two\nlines");
    }

    #[test]
    fn what_is_written_reads_back_identical() {
        let hit = Hit::new("a", Channel::Vector, 3, 0.125);
        let mut out = Vec::new();
        write_line(&mut out, &hit).unwrap();
        let back: Vec<Hit> = read(out.as_slice()).collect::<Result<_, _>>().unwrap();
        assert_eq!(back, vec![hit]);
    }

    #[test]
    fn every_record_is_flushed_as_it_is_written() {
        #[derive(Default)]
        struct Counting {
            flushes: usize,
        }
        impl Write for Counting {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.flushes += 1;
                Ok(())
            }
        }
        let mut out = Counting::default();
        for i in 0..3 {
            write_line(&mut out, &Hit::new("a", Channel::Meta, i, 1.0)).unwrap();
        }
        assert_eq!(out.flushes, 3);
    }

    #[test]
    fn a_closed_consumer_is_reported_as_a_broken_pipe_not_as_bad_data() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(ErrorKind::BrokenPipe, "broken pipe"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let e = write_line(&mut Closed, &Hit::new("a", Channel::Meta, 1, 1.0)).unwrap_err();
        assert!(e.is_broken_pipe(), "got {e:?}");
    }
}
