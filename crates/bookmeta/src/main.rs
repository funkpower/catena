//! The metadata a file already carries.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::exit;

/// Read what a file says about itself, one record per file.
///
/// Every ebook format has somewhere to put the title, and almost every file
/// in a library has something written there. Reading it costs nothing and
/// needs no network, and it is the first thing a catalogue wants.
///
/// What comes back is what the file says, not what is true. A PDF titled
/// `Microsoft Word - ch3.doc` is reported with that title and a note saying
/// where it came from.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 every file said something, 1 at least one said nothing,
74 a file could not be read.

Example:

    bookmeta library/*.epub | jq -r '[.title, (.authors|join(\", \"))] | @tsv'
    bookmeta --prompt scan.pdf | ollama run qwen3:8b | bookmeta --merge scan.pdf"
)]
struct Args {
    /// Files to read.
    paths: Vec<PathBuf>,

    /// Write the question to put to a model about what the file did not say,
    /// instead of the record. Nothing is sent: the prompt goes to stdout.
    #[arg(long)]
    prompt: bool,

    /// Read a model's reply on stdin and fill in what was missing, writing
    /// the completed record.
    #[arg(long)]
    merge: bool,

    /// Text to show the model, where the file itself is not worth quoting.
    /// Defaults to nothing, which asks about the metadata alone.
    #[arg(long, value_name = "FILE")]
    excerpt: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };
    if args.paths.is_empty() {
        eprintln!("bookmeta: no files given");
        return ExitCode::from(exit::USAGE);
    }
    if args.prompt && args.merge {
        eprintln!("bookmeta: --prompt composes a question and --merge answers one");
        return ExitCode::from(exit::USAGE);
    }

    let excerpt = match args.excerpt.as_deref() {
        None => None,
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(e) => {
                eprintln!("bookmeta: {}: {e}", path.display());
                return ExitCode::from(exit::IO);
            }
        },
    };

    // Both of the model-facing modes are about one file: a prompt for two
    // books at once gets an answer about neither.
    if args.prompt || args.merge {
        if args.paths.len() != 1 {
            eprintln!("bookmeta: --prompt and --merge take one file at a time");
            return ExitCode::from(exit::USAGE);
        }
        return one(&args, &args.paths[0], excerpt.as_deref());
    }

    let mut records = Vec::with_capacity(args.paths.len());
    let mut silent = 0usize;
    for path in &args.paths {
        let meta = bookmeta::read(path);
        if meta.is_empty() {
            silent += 1;
        }
        records.push(meta);
    }

    let wrote = catena_cli::stream("bookmeta", &records);
    if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
        return wrote;
    }
    ExitCode::from(match silent {
        0 => exit::OK,
        _ => exit::NO,
    })
}

/// The two modes that put a question to somebody else's model.
fn one(args: &Args, path: &std::path::Path, excerpt: Option<&str>) -> ExitCode {
    let mut meta = bookmeta::read(path);

    if args.prompt {
        return match bookmeta::prompt::compose(&meta, excerpt) {
            // Nothing missing is not a failure, and printing a prompt
            // nobody needs would waste a model's time.
            None => ExitCode::from(exit::NO),
            Some(prompt) => catena_cli::text("bookmeta", &prompt),
        };
    }

    let reply = match catena_cli::read_text(None) {
        Ok(reply) => reply,
        Err((code, why)) => {
            eprintln!("bookmeta: {why}");
            return ExitCode::from(code);
        }
    };
    match bookmeta::prompt::merge(&mut meta, &reply) {
        Err(why) => {
            eprintln!("bookmeta: {why}");
            ExitCode::from(exit::DATA)
        }
        Ok(filled) => {
            if filled.is_empty() {
                eprintln!("bookmeta: the reply filled nothing in");
            }
            catena_cli::stream("bookmeta", std::slice::from_ref(&meta))
        }
    }
}
