//! What a catalogue says about an identifier.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use corpus_core::exit;

/// Ask the body that assigns an identifier what it knows about one.
///
/// `idnorm` can say a DOI is well formed. Only a register can say the paper
/// exists — that is a fact about the world rather than about the string, and
/// it is what catches a well-formed DOI a catalogue has been carrying wrongly
/// for three years.
///
/// Crossref for a DOI, the arXiv for a preprint, OpenLibrary for a book. This
/// is the only tool here besides `emb` that touches the network, and it is the
/// whole of what it does.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 every identifier was found, 1 at least one was not,
65 the input was not identifiers, 69 a register could not be reached,
74 the file could not be read.

Example:

    idnorm refs.txt | bibfetch | jq -r 'select(.found) | .title'
    idnorm --kind doi refs.txt | bibfetch --polite you@example.org"
)]
struct Args {
    /// Identifier records to look up, as `idnorm` writes them. Reads stdin
    /// when absent.
    path: Option<PathBuf>,

    /// An address to put in the request, which the registers ask for and
    /// answer faster when they get.
    #[arg(long, value_name = "EMAIL")]
    polite: Option<String>,

    /// How long to wait on a register, in seconds.
    #[arg(long, default_value_t = bibfetch::TIMEOUT.as_secs(), value_name = "SECONDS")]
    timeout: u64,

    /// How many times to ask before believing a failure.
    #[arg(long, default_value_t = bibfetch::ATTEMPTS, value_name = "COUNT")]
    attempts: u32,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("bibfetch: {why}");
            return ExitCode::from(code);
        }
    };

    let agent = bibfetch::ask::Ask {
        polite: args.polite.clone(),
        timeout: Duration::from_secs(args.timeout),
        attempts: args.attempts,
    }
    .agent();

    let mut records = Vec::new();
    let mut missing = 0usize;

    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (id, kind) = match read_identifier(line) {
            Ok(pair) => pair,
            Err(why) => {
                eprintln!("bibfetch: line {}: {why}", number + 1);
                return ExitCode::from(exit::DATA);
            }
        };

        match bibfetch::ask::lookup(&agent, &id, &kind, args.attempts) {
            Ok(None) => {
                eprintln!("bibfetch: {id}: no register here answers about {kind} identifiers");
                missing += 1;
            }
            Ok(Some(record)) => {
                if !record.found {
                    missing += 1;
                }
                records.push(record);
            }
            Err(why) => {
                eprintln!("bibfetch: {id}: {why}");
                return ExitCode::from(exit::UNAVAILABLE);
            }
        }
    }

    let wrote = catena_cli::stream("bibfetch", &records);
    if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
        return wrote;
    }
    ExitCode::from(match missing {
        0 => exit::OK,
        _ => exit::NO,
    })
}







/// The identifier and its kind out of one record.
fn read_identifier(line: &str) -> Result<(String, String), String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("not a record: {e}"))?;
    let id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or("no id")?
        .to_string();
    let kind = value
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or("no kind")?
        .to_string();
    Ok((id, kind))
}
