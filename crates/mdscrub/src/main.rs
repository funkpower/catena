//! Cleans converted markdown.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Clean converted markdown: drop residual HTML and control characters,
/// make headings readable, and spell every page boundary the same way.
///
/// Reads markdown on stdin, or from a path, and writes it to stdout. Safe to
/// run twice.
///
/// Page boundaries are left alone unless `--pages` asks for them, because
/// only the program that just read a PDF knows that a form feed in the text
/// is a page break rather than a stray control character.
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

    /// First turn page separators and form feeds into page markers. For text
    /// straight out of a PDF or DjVu extractor, which marks its page
    /// boundaries that way.
    #[arg(long)]
    pages: bool,
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

    let markdown = match args.pages {
        true => mdscrub::pages::normalize(&markdown),
        false => markdown,
    };
    catena_cli::text("mdscrub", &mdscrub::scrub(&markdown))
}
