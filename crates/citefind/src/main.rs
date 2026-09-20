//! The references in a bibliography or a page of notes.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::exit;

/// Read a reference list, one record per reference.
///
/// The input is the bibliography, works-cited list, or page of numbered
/// notes as text. It is cut into entries — at the note numbers where there
/// are any, at the layout where there are not — and each entry is read as far
/// as its punctuation allows.
///
/// Fields that could not be read off the entry are left empty and named in
/// `unsettled`, because a wrong year and a real one are indistinguishable
/// downstream and a missing one is not. `--prompt` writes a question about
/// exactly those, for a model of your choosing; nothing is sent from here.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 it found references, 1 it found none, 65 the input was not
readable as text, 74 the file could not be read.

Example:

    citefind bibliography.md | jq -r '[.year, .title] | @tsv'
    mdsplit book.md | citefind --prompt > ask.txt
    citefind --prompt refs.md | ollama run qwen3:8b > reply.json
    citefind --merge reply.json refs.md"
)]
struct Args {
    /// The reference list to read. Standard input when not given.
    path: Option<PathBuf>,

    /// Say nothing; the exit code is the answer.
    #[arg(long, short)]
    quiet: bool,

    /// Write the question to put to a model about the entries that did not
    /// parse, instead of the records. Nothing is sent: it goes to stdout.
    #[arg(long)]
    prompt: bool,

    /// Fill in what would not parse from a model's reply, written earlier by
    /// `--prompt`, and write the completed records.
    #[arg(long, value_name = "FILE")]
    merge: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };
    if args.prompt && args.merge.is_some() {
        eprintln!("citefind: --prompt composes a question and --merge answers one");
        return ExitCode::from(exit::USAGE);
    }

    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("citefind: {why}");
            return ExitCode::from(code);
        }
    };

    let mut references = citefind::read(&text);
    if references.is_empty() {
        if !args.quiet {
            eprintln!("citefind: nothing in this looks like a reference");
        }
        return ExitCode::from(exit::NO);
    }

    if let Some(path) = args.merge.as_deref() {
        let reply = match std::fs::read_to_string(path) {
            Ok(reply) => reply,
            Err(e) => {
                eprintln!("citefind: {}: {e}", path.display());
                return ExitCode::from(exit::IO);
            }
        };
        match citefind::prompt::merge(&mut references, &reply) {
            Err(why) => {
                eprintln!("citefind: {why}");
                return ExitCode::from(exit::DATA);
            }
            Ok(0) => eprintln!("citefind: the reply filled nothing in"),
            Ok(filled) => eprintln!("citefind: {filled} entries filled in from the reply"),
        }
    }

    if args.quiet {
        return ExitCode::from(exit::OK);
    }
    if args.prompt {
        return match citefind::prompt::compose(&references) {
            // Nothing unsettled is not a failure, and a prompt nobody needs
            // would waste a model's time.
            None => ExitCode::from(exit::NO),
            Some(prompt) => catena_cli::text("citefind", &prompt),
        };
    }
    catena_cli::stream("citefind", &references)
}
