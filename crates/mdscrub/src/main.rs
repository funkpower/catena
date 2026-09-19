//! Cleans converted markdown.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

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
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let markdown = match catena_cli::read_text(args.path.as_deref()) {
        Ok(markdown) => markdown,
        Err((code, why)) => {
            eprintln!("mdscrub: {why}");
            return ExitCode::from(code);
        }
    };

    catena_cli::text("mdscrub", &mdscrub::scrub(&markdown))
}
