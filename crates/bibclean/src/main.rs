//! Bibliographic hygiene over catalogue records.

use std::path::PathBuf;
use std::process::ExitCode;

use bibclean::Book;
use clap::Parser;
use corpus_core::{exit, ndjson};

/// Audit catalogue records and repair what can be repaired without a lookup.
///
/// Reads one JSON record per line — `title`, `author`, `isbn`, `path`,
/// `slug`, `id` and `front_matter` are read, everything else is carried
/// through untouched — and writes each one back with the faults it has, how
/// much they weigh, and a line for every change made and why.
///
/// Nothing is rewritten silently. A run can be read before it is agreed to,
/// which is the difference between cleaning a catalogue and damaging it in a
/// new way.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 nothing needed attention, 1 something did, 65 a record could
not be read.

Example:

    calibredb list --for-machine | jq -c '.[]' | bibclean --flagged"
)]
struct Args {
    /// Record stream to read. Reads stdin when absent.
    path: Option<PathBuf>,

    /// Write only the records that need attention. A missing ISBN alone does
    /// not count as needing attention: nearly every book lacks one.
    #[arg(long)]
    flagged: bool,
}

/// Whether anything about a record wants a person's attention.
///
/// A missing ISBN alone does not: nearly every book lacks one. A slug that
/// should be replaced does, even when nothing else is wrong, because the
/// identifier is the part other things have written down.
fn needs_attention(cleaned: &bibclean::Cleaned) -> bool {
    bibclean::reasons::worth_fixing(&cleaned.reasons)
        || cleaned.slug_proposal.is_some()
        || cleaned.author_proposal.is_some()
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("bibclean: {why}");
            return ExitCode::from(code);
        }
    };

    let books: Vec<Book> = match ndjson::read(text.as_bytes()).collect() {
        Ok(books) => books,
        Err(e) => {
            eprintln!("bibclean: {e}");
            return ExitCode::from(exit::DATA);
        }
    };

    let mut flagged = false;
    let cleaned: Vec<bibclean::Cleaned> = books
        .iter()
        .map(bibclean::clean)
        .inspect(|cleaned| flagged |= needs_attention(cleaned))
        .filter(|cleaned| !args.flagged || needs_attention(cleaned))
        .collect();

    let wrote = catena_cli::stream("bibclean", &cleaned);
    if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
        return wrote;
    }
    ExitCode::from(if flagged { exit::NO } else { exit::OK })
}
