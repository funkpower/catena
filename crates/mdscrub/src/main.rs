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
    long_about = None,
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
        Err(e) => {
            eprintln!("mdscrub: {e}");
            return ExitCode::from(exit::IO);
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

fn read(path: Option<&Path>) -> std::io::Result<String> {
    match path {
        Some(path) => std::fs::read_to_string(path),
        None => {
            let mut markdown = String::new();
            std::io::stdin().read_to_string(&mut markdown)?;
            Ok(markdown)
        }
    }
}
