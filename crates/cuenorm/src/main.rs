//! Any subtitle format in, one normalised cue stream out.

use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use corpus_core::{exit, ndjson};

/// Read a subtitle file — SubRip, WebVTT or SubStation Alpha — and write one
/// cue per line as JSON.
///
/// The format is worked out from the content, not from the name, because a
/// subtitle file is named for the format somebody expected it to be in.
/// Markup and bracketed sound cues are taken out, and consecutive cues with
/// identical text are merged, which is what automatic captioning needs: it
/// repeats each line across overlapping windows, so the same sentence arrives
/// three or four times over.
#[derive(Parser)]
#[command(
    version,
    about,
    long_about = None,
    after_help = "\
Example:

    cuenorm episode.vtt | cuepack --seconds 60 > passages.ndjson"
)]
struct Args {
    /// Subtitle file to read. Reads stdin when absent.
    path: Option<PathBuf>,

    /// Keep consecutive repeats instead of merging them.
    #[arg(long)]
    keep_repeats: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let content = match read(args.path.as_deref()) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("cuenorm: {e}");
            return ExitCode::from(exit::IO);
        }
    };

    let cues = cuekit::parse(&content);
    let cues = if args.keep_repeats {
        cues
    } else {
        cuekit::merge_repeats(&cues)
    };

    let mut out = BufWriter::new(std::io::stdout().lock());
    for cue in &cues {
        if let Err(e) = ndjson::write_line(&mut out, cue) {
            // A consumer that stopped reading is not a failure.
            if e.is_broken_pipe() {
                return ExitCode::from(exit::OK);
            }
            eprintln!("cuenorm: {e}");
            return ExitCode::from(exit::IO);
        }
    }
    match out.flush() {
        Ok(()) => ExitCode::from(exit::OK),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::from(exit::OK),
        Err(e) => {
            eprintln!("cuenorm: {e}");
            ExitCode::from(exit::IO)
        }
    }
}

/// Read the file as text, tolerating the encodings a subtitle file arrives in.
///
/// Not UTF-8 is common: a subtitle file downloaded from anywhere is as likely
/// to be Latin-1, and refusing it would make the tool useless for exactly the
/// archives that need it. Undecodable bytes become the replacement character
/// rather than an error, because a cue with one wrong character still has its
/// timing and most of its words.
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
