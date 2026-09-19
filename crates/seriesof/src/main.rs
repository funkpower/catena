//! The series a book belongs to, and which volume of it this is.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::exit;

/// Read a series and a volume off a title, one record per proposal.
///
/// A shelf groups by series and volume, and most catalogues have neither. Four
/// records all reading "A History of Philosophy" with nothing to say which
/// volume any of them is, is what that looks like from the reading chair.
///
/// Reads one JSON record per line with a `title` and optionally an `author`,
/// or plain titles with `--titles`. Says nothing about the books it cannot
/// place.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 something was proposed, 1 nothing was, 65 the input was not
records, 74 the file could not be read.

Example:

    seriesof catalogue.ndjson | jq -r '\"\\(.series) #\\(.volume // \"-\")\"'
    ls *.epub | sed 's/\\.epub$//' | seriesof --titles"
)]
struct Args {
    /// Records to read. Reads stdin when absent.
    path: Option<PathBuf>,

    /// Read plain titles, one per line, rather than records.
    #[arg(long)]
    titles: bool,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("seriesof: {why}");
            return ExitCode::from(code);
        }
    };

    let mut proposals = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (title, author) = match args.titles {
            true => (line.trim().to_string(), None),
            false => match read_record(line) {
                Ok(pair) => pair,
                Err(why) => {
                    eprintln!("seriesof: line {}: {why}", number + 1);
                    return ExitCode::from(exit::DATA);
                }
            },
        };
        if let Some(proposal) = seriesof::series_of(&title, author.as_deref()) {
            proposals.push(proposal);
        }
    }

    // Nothing to propose is an answer, and the usual one for a catalogue that
    // has already been through this.
    if proposals.is_empty() {
        return ExitCode::from(exit::NO);
    }
    catena_cli::stream("seriesof", &proposals)
}

/// The title and author out of one record.
fn read_record(line: &str) -> Result<(String, Option<String>), String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("not a record: {e}"))?;
    let title = value
        .get("title")
        .and_then(serde_json::Value::as_str)
        .ok_or("no title")?
        .to_string();
    let author = value
        .get("author")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    Ok((title, author))
}
