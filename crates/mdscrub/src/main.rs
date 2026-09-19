//! Cleans converted markdown.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use corpus_core::exit;

/// Clean converted markdown: drop residual HTML and control characters,
/// make headings readable, and spell every page boundary the same way.
///
/// Reads markdown on stdin, or from a path, and writes it to stdout. Safe to
/// run twice.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Example:

    pandoc -t commonmark book.epub | mdscrub | chunkmd > chunks.ndjson"
)]
struct Args {
    /// Markdown file to read. Reads stdin when absent.
    path: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let markdown = match read(args.path.as_deref()) {
        Ok(markdown) => markdown,
        Err((code, why)) => {
            eprintln!("mdscrub: {why}");
            return ExitCode::from(code);
        }
    };

    let cleaned = mdscrub::scrub(&markdown);
    match std::io::stdout().write_all(cleaned.as_bytes()) {
        Ok(()) => ExitCode::from(exit::OK),
        // A consumer that stopped reading is not a failure.
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::from(exit::OK),
        Err(e) => {
            eprintln!("mdscrub: {e}");
            ExitCode::from(exit::IO)
        }
    }
}

/// Read the input, distinguishing a file that cannot be read from one that
/// is not text: the first is an I/O failure and the second is bad input, and
/// a caller routing on the exit code needs to tell them apart.
fn read(path: Option<&Path>) -> Result<String, (u8, String)> {
    let bytes = match path {
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
    }?;
    String::from_utf8(bytes).map_err(|e| (exit::DATA, format!("input is not UTF-8: {e}")))
}
