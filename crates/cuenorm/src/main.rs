//! Any subtitle format in, one normalised cue stream out.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

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
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let content = match catena_cli::read_lossy(args.path.as_deref()) {
        Ok(content) => content,
        Err((code, why)) => {
            eprintln!("cuenorm: {why}");
            return ExitCode::from(code);
        }
    };

    let cues = cuekit::parse(&content);
    let cues = if args.keep_repeats {
        cues
    } else {
        cuekit::merge_repeats(&cues)
    };

    catena_cli::stream("cuenorm", &cues)
}
