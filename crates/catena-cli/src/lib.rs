//! The parts of a filter that every tool here does the same way.
//!
//! Not a framework — three functions, each of which exists because getting
//! it wrong is easy and the consequence is a pipeline that misreports why it
//! stopped:
//!
//! - a usage error exits 64, not whatever the argument parser felt like;
//! - a consumer that closed the pipe is the end of the job, not a failure;
//! - a record is flushed as it is written, so a slow stage downstream sees it
//!   immediately.

use std::io::{BufWriter, Write};
use std::process::ExitCode;

use corpus_core::{exit, ndjson};
use serde::Serialize;

/// Parse the command line, or exit.
///
/// `--help` and `--version` are not errors and leave with 0; anything else
/// wrong with the arguments leaves with 64, which is what the rest of the
/// suite means by "the command line was wrong".
pub fn parse<T: clap::Parser>() -> Result<T, ExitCode> {
    T::try_parse().map_err(|e| {
        let _ = e.print();
        match e.use_stderr() {
            true => ExitCode::from(exit::USAGE),
            false => ExitCode::from(exit::OK),
        }
    })
}

/// Write records to stdout as NDJSON, one per line, flushed as they go.
pub fn stream<'a, T: Serialize + 'a>(
    tool: &str,
    records: impl IntoIterator<Item = &'a T>,
) -> ExitCode {
    let mut out = BufWriter::new(std::io::stdout().lock());
    for record in records {
        match ndjson::write_line(&mut out, record) {
            Ok(()) => {}
            // A consumer that stopped reading is not a failure: `… | head` is
            // a normal way to look at a stream.
            Err(e) if e.is_broken_pipe() => return ExitCode::from(exit::OK),
            Err(e) => {
                eprintln!("{tool}: {e}");
                return ExitCode::from(exit::IO);
            }
        }
    }
    finish(tool, out)
}

/// Write text to stdout and leave.
pub fn text(tool: &str, text: &str) -> ExitCode {
    let mut out = BufWriter::new(std::io::stdout().lock());
    match out.write_all(text.as_bytes()) {
        Ok(()) => finish(tool, out),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::from(exit::OK),
        Err(e) => {
            eprintln!("{tool}: {e}");
            ExitCode::from(exit::IO)
        }
    }
}

/// Flush what is buffered, treating a closed consumer as the end of the job.
pub fn finish(tool: &str, mut out: impl Write) -> ExitCode {
    match out.flush() {
        Ok(()) => ExitCode::from(exit::OK),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::from(exit::OK),
        Err(e) => {
            eprintln!("{tool}: {e}");
            ExitCode::from(exit::IO)
        }
    }
}

/// Read the whole input as text, telling a file that cannot be read from one
/// that is not text.
///
/// The two are different answers: the first is an I/O failure and the second
/// is bad input, and a caller routing on the exit code needs to tell them
/// apart.
pub fn read_text(path: Option<&std::path::Path>) -> Result<String, (u8, String)> {
    let bytes = read_bytes(path)?;
    String::from_utf8(bytes).map_err(|e| (exit::DATA, format!("input is not UTF-8: {e}")))
}

/// Read the whole input, replacing anything that is not UTF-8.
///
/// For the formats that routinely arrive in another encoding — a subtitle
/// file downloaded from anywhere is as likely to be Latin-1 — where a cue
/// with one wrong character still has its timing and most of its words.
pub fn read_lossy(path: Option<&std::path::Path>) -> Result<String, (u8, String)> {
    Ok(String::from_utf8_lossy(&read_bytes(path)?).into_owned())
}

fn read_bytes(path: Option<&std::path::Path>) -> Result<Vec<u8>, (u8, String)> {
    use std::io::Read as _;

    match path {
        Some(path) => {
            std::fs::read(path).map_err(|e| (exit::IO, format!("{}: {e}", path.display())))
        }
        None => {
            let mut bytes = Vec::new();
            std::io::stdin()
                .read_to_end(&mut bytes)
                .map(|_| bytes)
                .map_err(|e| (exit::IO, e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_a_file_that_is_not_there_is_an_io_failure() {
        let missing = std::path::Path::new("/definitely/not/here");
        let (code, _) = read_text(Some(missing)).expect_err("should fail");
        assert_eq!(code, exit::IO);
    }

    #[test]
    fn reading_bytes_that_are_not_text_is_a_data_failure() {
        let path = std::env::temp_dir().join("catena-cli-not-text");
        std::fs::write(&path, [0xff, 0xfe]).expect("a writable path");
        let (code, why) = read_text(Some(&path)).expect_err("should fail");
        assert_eq!(code, exit::DATA);
        assert!(why.contains("UTF-8"), "{why}");
    }

    #[test]
    fn the_lossy_read_takes_what_the_strict_one_refuses() {
        let path = std::env::temp_dir().join("catena-cli-lossy");
        std::fs::write(&path, [b'a', 0xff, b'b']).expect("a writable path");
        assert_eq!(read_lossy(Some(&path)).expect("should read"), "a\u{fffd}b");
    }

    #[test]
    fn a_closed_consumer_is_the_end_of_the_job_and_not_a_failure() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Ok(0)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "broken pipe",
                ))
            }
        }
        assert_eq!(
            format!("{:?}", finish("test", Closed)),
            format!("{:?}", ExitCode::from(exit::OK))
        );
    }
}
