//! The record a book prints in its own front matter.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::exit;

/// Read what a book prints about itself in its opening pages.
///
/// The input is the book as markdown — the whole of it, or as much of the
/// front as you care to pass. The title page and the cataloguing block on
/// the copyright page are read, and what they state comes back as one
/// record, with the page each field was read off named in `read_from`.
///
/// Nothing is inferred. A scan that starts at chapter one prints no title
/// page, and what comes back then is a record with nothing in it and every
/// field named in `unsettled` — which is the answer, and a usable one.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 the book printed something about itself, 1 it printed nothing
this could read, 65 the input was not readable as text, 74 the file could not
be read.

Example:

    colophon book.md | jq -r '[.title, (.authors|join(\", \"))] | @tsv'
    colophon *.md | jq -r 'select(.unsettled | index(\"title\")) | .path'
    colophon book.md | jq -r '.isbns[]' | idnorm | bibfetch"
)]
struct Args {
    /// The books to read. Standard input when none are given.
    paths: Vec<PathBuf>,

    /// Say nothing; the exit code is the answer.
    #[arg(long, short)]
    quiet: bool,

    /// Write the question to put to a model about the fields these pages
    /// did not settle, instead of the record. Nothing is sent: it goes to
    /// stdout. One book at a time, because the pages differ.
    #[arg(long)]
    prompt: bool,

    /// Fill in what the pages did not settle from a model's reply, written
    /// earlier by `--prompt`, and write the completed record.
    #[arg(long, value_name = "FILE")]
    merge: Option<PathBuf>,

    /// How many pages in from the front to read.
    #[arg(long, value_name = "COUNT", default_value_t = colophon::PAGES)]
    pages: usize,

    /// A closed list of headings to file the book's printed subjects under.
    ///
    /// One heading per line, optionally `Heading = term, term` to give the
    /// words that should match it; a heading with no terms matches its own
    /// name. A cataloguing block states subjects in Library of Congress form,
    /// which is no use as a shelf, so this says what the shelf is. What
    /// matches nothing comes back under `unshelved` rather than being dropped.
    #[arg(long, value_name = "FILE")]
    shelf: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };
    if args.pages == 0 {
        eprintln!("colophon: --pages 0 would read nothing");
        return ExitCode::from(exit::USAGE);
    }
    if args.prompt && args.merge.is_some() {
        eprintln!("colophon: --prompt composes a question and --merge answers one");
        return ExitCode::from(exit::USAGE);
    }
    if (args.prompt || args.merge.is_some()) && args.paths.len() > 1 {
        eprintln!("colophon: one book at a time for --prompt and --merge");
        return ExitCode::from(exit::USAGE);
    }

    // Read the shelf before any book, so a bad path fails at once rather than
    // after the whole library has been read with no shelf applied.
    let shelf = match args.shelf.as_deref() {
        None => None,
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => {
                let s = colophon::shelf::Shelf::parse(&text);
                if s.is_empty() {
                    eprintln!("colophon: {} lists no headings", path.display());
                    return ExitCode::from(exit::USAGE);
                }
                Some(s)
            }
            Err(e) => {
                eprintln!("colophon: {}: {e}", path.display());
                return ExitCode::from(exit::USAGE);
            }
        },
    };

    let inputs: Vec<Option<PathBuf>> = if args.paths.is_empty() {
        vec![None]
    } else {
        args.paths.iter().cloned().map(Some).collect()
    };

    let mut records = Vec::with_capacity(inputs.len());
    let mut last_text = String::new();
    for path in inputs {
        let text = match catena_cli::read_text(path.as_deref()) {
            Ok(text) => text,
            Err((code, why)) => {
                eprintln!("colophon: {why}");
                return ExitCode::from(code);
            }
        };
        let mut record = colophon::read_onto(&text, args.pages, shelf.as_ref());
        record.path = path.as_deref().map(|p| p.display().to_string());
        records.push(record);
        last_text = text;
    }

    if args.prompt {
        let record = &records[0];
        return match colophon::prompt::compose(record, &last_text, args.pages) {
            // Nothing left to ask is not a failure, and a question nobody
            // needs would waste a model's time.
            None => ExitCode::from(exit::NO),
            Some(question) => catena_cli::text("colophon", &question),
        };
    }
    if let Some(path) = args.merge.as_deref() {
        let reply = match std::fs::read_to_string(path) {
            Ok(reply) => reply,
            Err(e) => {
                eprintln!("colophon: {}: {e}", path.display());
                return ExitCode::from(exit::IO);
            }
        };
        match colophon::prompt::merge(&mut records[0], &reply) {
            Err(why) => {
                eprintln!("colophon: {why}");
                return ExitCode::from(exit::DATA);
            }
            Ok(0) => eprintln!("colophon: the reply filled nothing in"),
            Ok(filled) => eprintln!("colophon: {filled} field(s) filled in from the reply"),
        }
    }

    let anything = records.iter().any(|r| !r.is_empty());
    if args.quiet {
        return ExitCode::from(if anything { exit::OK } else { exit::NO });
    }
    if !anything {
        eprintln!("colophon: nothing in these pages says what the book is");
        for record in &records {
            let _ = catena_cli::stream("colophon", std::slice::from_ref(record));
        }
        return ExitCode::from(exit::NO);
    }
    catena_cli::stream("colophon", &records)
}
