//! Packs cues into passages worth embedding.

use std::io::{BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::{exit, ndjson};
use cuekit::Cue;
use cuepack::Budgets;

/// Pack a cue stream into passages, one chunk record per line.
///
/// Takes either the cue stream `cuenorm` writes or a subtitle file directly.
/// Each passage keeps the start of its first cue and the end of its last, so
/// a result can be cited to the second — the same record a book's chunks use,
/// with times where they have pages.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Example:

    cuenorm episode.vtt | cuepack --seconds 75 --max-tokens 512 > passages.ndjson"
)]
struct Args {
    /// Cue stream or subtitle file to read. Reads stdin when absent.
    path: Option<PathBuf>,

    /// Flush a passage once it covers this much audio.
    #[arg(long, default_value_t = Budgets::default().seconds, value_name = "SECONDS")]
    seconds: f64,

    /// Never let a passage exceed this many tokens.
    #[arg(long, default_value_t = Budgets::default().max_tokens, value_name = "TOKENS")]
    max_tokens: usize,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let budgets = Budgets {
        seconds: args.seconds,
        max_tokens: args.max_tokens,
    };
    if !budgets.seconds.is_finite() || budgets.seconds <= 0.0 || budgets.max_tokens == 0 {
        eprintln!("cuepack: --seconds and --max-tokens must be above zero");
        return ExitCode::from(exit::USAGE);
    }

    let content = match read(args.path.as_deref()) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("cuepack: {e}");
            return ExitCode::from(exit::IO);
        }
    };

    let cues = match cues(&content) {
        Ok(cues) => cues,
        Err(e) => {
            eprintln!("cuepack: {e}");
            return ExitCode::from(exit::DATA);
        }
    };

    let mut out = BufWriter::new(std::io::stdout().lock());
    for passage in cuepack::pack(&cues, &budgets) {
        if let Err(e) = ndjson::write_line(&mut out, &passage) {
            // A consumer that stopped reading is not a failure.
            if e.is_broken_pipe() {
                return ExitCode::from(exit::OK);
            }
            eprintln!("cuepack: {e}");
            return ExitCode::from(exit::IO);
        }
    }
    match out.flush() {
        Ok(()) => ExitCode::from(exit::OK),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::from(exit::OK),
        Err(e) => {
            eprintln!("cuepack: {e}");
            ExitCode::from(exit::IO)
        }
    }
}

/// Read cues from either encoding: a cue stream begins with a JSON object
/// and a subtitle file does not.
fn cues(content: &str) -> Result<Vec<Cue>, ndjson::Error> {
    let first = content
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    if first.trim_start().starts_with('{') {
        return ndjson::read(content.as_bytes()).collect();
    }
    Ok(cuekit::parse(content))
}

fn read(path: Option<&std::path::Path>) -> std::io::Result<String> {
    let bytes = match path {
        Some(path) => std::fs::read(path)?,
        None => {
            let mut bytes = Vec::new();
            std::io::stdin().read_to_end(&mut bytes)?;
            bytes
        }
    };
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
