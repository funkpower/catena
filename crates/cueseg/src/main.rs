//! Turns a timed transcript into topic chapters.

use std::path::PathBuf;
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

    /// One vector per window, as a JSON array per line, in window order.
    ///
    /// Without this the windows are compared by the words they repeat, which
    /// cannot tell that "ships" and "vessels" are the same subject. With it
    /// they are compared however the vectors were made. `--windows` prints
    /// the text to embed.
    #[arg(long, value_name = "FILE")]
    embeddings: Option<PathBuf>,

    /// Print the window text, one per line, and stop.
    ///
    /// What to embed, in the order `--embeddings` expects it back.
    #[arg(long)]
    windows: bool,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let content = match catena_cli::read_lossy(args.path.as_deref()) {
        Ok(content) => content,
        Err((code, why)) => {
            eprintln!("cueseg: {why}");
            return ExitCode::from(code);
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

    if args.windows {
        let text: String = cueseg::windows_of(&cues)
            .iter()
            .map(|window| format!("{}\n", window.text.replace('\n', " ")))
            .collect();
        return catena_cli::text("cueseg", &text);
    }

    let embeddings = match args.embeddings.as_deref() {
        None => None,
        Some(path) => match read_embeddings(path) {
            Ok(vectors) => Some(vectors),
            Err((code, why)) => {
                eprintln!("cueseg: {why}");
                return ExitCode::from(code);
            }
        },
    };

    let windows = cueseg::windows_of(&cues).len();
    if let Some(vectors) = embeddings.as_deref() {
        if vectors.len() != windows {
            eprintln!(
                "cueseg: {} vectors for {windows} windows — run --windows to see what to embed",
                vectors.len()
            );
            return ExitCode::from(exit::DATA);
        }
    }

    let chapters = cueseg::segment_with(&cues, duration, embeddings.as_deref());
    catena_cli::stream("cueseg", &chapters)
}

/// One vector per line, as `emb` writes them.
fn read_embeddings(path: &std::path::Path) -> Result<Vec<Vec<f64>>, (u8, String)> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| (exit::IO, format!("{}: {e}", path.display())))?;
    let mut out = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let vector: Vec<f64> = serde_json::from_str(line).map_err(|e| {
            (
                exit::DATA,
                format!("{}: line {}: {e}", path.display(), number + 1),
            )
        })?;
        out.push(vector);
    }
    Ok(out)
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
