//! Turns a timed transcript into topic chapters.

use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use corpus_core::{exit, ndjson};
use cuekit::Cue;

/// Read a transcript and write its topic chapters, one per line as JSON.
///
/// Takes either the cue stream `cuenorm` writes or a subtitle file directly.
/// The chapters are found from the text alone — no model, no network — by
/// comparing what is said in one stretch with what is said in the next.
///
/// The titles are extracted from the chapters' own first sentences. They are
/// meant to be replaced by something better later, and to be usable until
/// they are.
#[derive(Parser)]
#[command(
    version,
    about,
    long_about = None,
    after_help = "\
Example:

    cuenorm episode.vtt | cueseg --duration 7245 > chapters.ndjson"
)]
struct Args {
    /// Cue stream or subtitle file to read. Reads stdin when absent.
    path: Option<PathBuf>,

    /// How long the recording is, in seconds. Defaults to the end of the
    /// last cue, which is earlier than the end of the file whenever the
    /// recording ends in music or silence.
    #[arg(long, value_name = "SECONDS")]
    duration: Option<f64>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let content = match read(args.path.as_deref()) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("cueseg: {e}");
            return ExitCode::from(exit::IO);
        }
    };

    let cues = match cues(&content) {
        Ok(cues) => cues,
        Err(e) => {
            eprintln!("cueseg: {e}");
            return ExitCode::from(exit::DATA);
        }
    };

    let duration = match args.duration {
        Some(duration) if duration.is_finite() && duration > 0.0 => duration,
        Some(duration) => {
            eprintln!("cueseg: --duration {duration} is not a length");
            return ExitCode::from(exit::USAGE);
        }
        None => cues.iter().map(|cue| cue.end).fold(0.0, f64::max),
    };

    let mut out = BufWriter::new(std::io::stdout().lock());
    for chapter in cueseg::segment(&cues, duration) {
        if let Err(e) = ndjson::write_line(&mut out, &chapter) {
            // A consumer that stopped reading is not a failure.
            if e.is_broken_pipe() {
                return ExitCode::from(exit::OK);
            }
            eprintln!("cueseg: {e}");
            return ExitCode::from(exit::IO);
        }
    }
    match out.flush() {
        Ok(()) => ExitCode::from(exit::OK),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::from(exit::OK),
        Err(e) => {
            eprintln!("cueseg: {e}");
            ExitCode::from(exit::IO)
        }
    }
}

/// Read cues from either encoding.
///
/// A cue stream begins with a JSON object; a subtitle file does not. That is
/// enough to tell them apart, and it saves a caller from having to say which
/// of the two it has.
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

fn read(path: Option<&Path>) -> std::io::Result<String> {
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
