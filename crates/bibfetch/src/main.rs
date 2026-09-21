//! What a catalogue says about an identifier.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use bibfetch::{Error, Source};
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

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(args.timeout)))
        .user_agent(user_agent(args.polite.as_deref()))
        .build()
        .into();

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

        let chain = Source::chain_for_kind(&kind);
        if chain.is_empty() {
            eprintln!("bibfetch: {id}: no register here answers about {kind} identifiers");
            missing += 1;
            continue;
        }

        // Ask the registers in turn and stop at the first complete answer.
        // An incomplete one is kept in case nothing better comes: a title
        // with no authors is still more than nothing, and a register being
        // down is only a failure when every register is.
        let mut best: Option<bibfetch::Record> = None;
        let mut last_error = None;
        for source in chain {
            match ask(&agent, *source, &id, &kind, args.attempts) {
                Ok(record) => {
                    let complete =
                        record.found && record.title.is_some() && !record.authors.is_empty();
                    let better = best.as_ref().is_none_or(|kept| record.found && !kept.found);
                    if complete {
                        best = Some(record);
                        break;
                    }
                    if better {
                        best = Some(record);
                    }
                }
                Err(why) => last_error = Some(why),
            }
        }

        match best {
            Some(record) => {
                if !record.found {
                    missing += 1;
                }
                records.push(record);
            }
            None => {
                let why = last_error
                    .map_or_else(|| "no register answered".to_string(), |e| e.to_string());
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

/// Ask a register, retrying a failure that might not be one.
///
/// Waits a second before the second attempt and two before the third: long
/// enough for a register that is rate-limiting, short enough that one which
/// is simply down does not hold up a pipeline.
fn ask(
    agent: &ureq::Agent,
    source: Source,
    id: &str,
    kind: &str,
    attempts: u32,
) -> Result<bibfetch::Record, Error> {
    let mut last = Error::Unreachable("no attempt was made".into());

    for attempt in 0..attempts.max(1) {
        match fetch(agent, source, id, kind) {
            Ok(record) => return Ok(record),
            // A shape this cannot read will not read any better next time.
            Err(e @ Error::Shape(_)) => return Err(e),
            Err(e) => last = e,
        }
        std::thread::sleep(Duration::from_secs(1 << attempt));
    }
    Err(last)
}

fn fetch(
    agent: &ureq::Agent,
    source: Source,
    id: &str,
    kind: &str,
) -> Result<bibfetch::Record, Error> {
    let response = agent.get(source.url(id)).call();
    let mut response = match response {
        Ok(response) => response,
        // A register that does not have it says so with a status, and that is
        // an answer rather than a failure.
        Err(ureq::Error::StatusCode(404)) => return Ok(bibfetch::missing(id, kind, source)),
        Err(e) => return Err(Error::Unreachable(e.to_string())),
    };
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::Unreachable(e.to_string()))?;
    source.read(id, kind, &body)
}

/// Who is asking.
///
/// The registers ask for an address and answer faster when they get one; a
/// tool that does not say who it is gets the slow pool.
fn user_agent(polite: Option<&str>) -> String {
    let version = env!("CARGO_PKG_VERSION");
    match polite {
        Some(address) => format!("bibfetch/{version} (mailto:{address})"),
        None => format!("bibfetch/{version}"),
    }
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
