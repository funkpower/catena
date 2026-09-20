//! A book's own table of contents.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::exit;

/// Read a book's table of contents, one record per entry.
///
/// A book usually knows its own chapters: an EPUB keeps them in a navigation
/// document, a PDF in an outline. Both are put there by the publisher, which
/// makes them better than any heading guessed at from converted text.
///
/// Entries come out in reading order, each carrying how deeply it is nested,
/// so a flat stream still has the shape of the book.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 it had one, 1 it had none, 65 it is not a format that keeps one,
74 the file could not be read.

Example:

    tocfind book.epub | jq -r '(\"  \" * .depth) + .title'
    for f in *.epub; do tocfind --quiet \"$f\" || echo \"no contents: $f\"; done"
)]
struct Args {
    /// The book to read.
    path: PathBuf,

    /// Say nothing; the exit code is the answer.
    #[arg(long, short)]
    quiet: bool,

    /// Print the contents as an indented list rather than as records.
    #[arg(long)]
    outline: bool,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let entries = match tocfind::read(&args.path) {
        Ok(entries) => entries,
        Err(why) => {
            if !args.quiet {
                eprintln!("tocfind: {}: {why}", args.path.display());
            }
            return ExitCode::from(match why {
                // Having none is an answer; not being able to have one is not.
                tocfind::Missing::None(_) => exit::NO,
                tocfind::Missing::Unsupported => exit::DATA,
                tocfind::Missing::Unreadable(_) => exit::IO,
            });
        }
    };

    if args.quiet {
        return ExitCode::from(exit::OK);
    }
    if args.outline {
        let text: String = entries
            .iter()
            .map(|entry| {
                let indent = "  ".repeat(entry.depth as usize);
                match entry.page {
                    Some(page) => format!("{indent}{}  ({page})\n", entry.title),
                    None => format!("{indent}{}\n", entry.title),
                }
            })
            .collect();
        return catena_cli::text("tocfind", &text);
    }
    catena_cli::stream("tocfind", &entries)
}
