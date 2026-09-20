//! What language a document is in.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::exit;

/// Say what language a text is in, one record per file.
///
/// The writing system settles most of the world outright; for the Latin and
/// Cyrillic alphabets the commonest words decide between what is left.
///
/// No model, no download, no network. A paragraph is plenty; a single line
/// often is not, and a close call is refused rather than guessed at.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 every file was placed, 1 at least one could not be,
74 a file could not be read.

Example:

    langid book.md
    langid converted/*.md | jq -r 'select(.language != \"en\") | [.path, .name] | @tsv'"
)]
struct Args {
    /// Files to read. Reads stdin when absent.
    paths: Vec<PathBuf>,

    /// Print the code alone, for one file.
    #[arg(long)]
    quiet: bool,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let mut verdicts = Vec::new();
    let mut unplaced = 0usize;

    if args.paths.is_empty() {
        let text = match catena_cli::read_lossy(None) {
            Ok(text) => text,
            Err((code, why)) => {
                eprintln!("langid: {why}");
                return ExitCode::from(code);
            }
        };
        verdicts.push(langid::detect("-", &text));
    } else {
        for path in &args.paths {
            let text = match catena_cli::read_lossy(Some(path)) {
                Ok(text) => text,
                Err((code, why)) => {
                    eprintln!("langid: {why}");
                    return ExitCode::from(code);
                }
            };
            verdicts.push(langid::detect(&path.display().to_string(), &text));
        }
    }
    unplaced += verdicts.iter().filter(|v| v.language.is_none()).count();

    if args.quiet {
        let codes: String = verdicts
            .iter()
            .map(|v| format!("{}\n", v.language.as_deref().unwrap_or("")))
            .collect();
        let wrote = catena_cli::text("langid", &codes);
        if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
            return wrote;
        }
        return ExitCode::from(match unplaced {
            0 => exit::OK,
            _ => exit::NO,
        });
    }

    let wrote = catena_cli::stream("langid", &verdicts);
    if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
        return wrote;
    }
    ExitCode::from(match unplaced {
        0 => exit::OK,
        _ => exit::NO,
    })
}
