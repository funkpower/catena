//! Whether a recording plays as it is, and the cheapest way to fix it if not.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use avplan::{Encoder, Streams};
use clap::Parser;
use corpus_core::exit;

/// Say whether a recording plays as it stands, and print the one `ffmpeg`
/// call that fixes it when it does not.
///
/// Runs nothing. What comes out is a command to look at, pipe somewhere, or
/// run — which is the point: converting is expensive and irreversible enough
/// to be worth reading first.
///
/// Shells out to `ffprobe` to read the streams and to `ffmpeg` to ask which
/// encoders this machine has. Both ship together.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 it needs converting, 1 it plays as it is, 2 could not tell,
69 ffmpeg or ffprobe is not installed, 74 the file could not be read.

Example:

    for f in *.webm; do avplan \"$f\" | xargs -d '\\n' ffmpeg; done
    avplan --json talk.mkv | jq -r '.reasons[]'"
)]
struct Args {
    /// The recording to judge.
    path: PathBuf,

    /// Emit a record instead of a command.
    #[arg(long)]
    json: bool,

    /// Where the converted copy should go. Defaults to beside the original.
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// The render node to encode on, where the encoder needs one named.
    #[arg(long, default_value = avplan::DEFAULT_VAAPI_DEVICE, value_name = "PATH")]
    vaapi_device: String,

    /// Use this encoder rather than the best one installed.
    #[arg(long, value_name = "NAME")]
    encoder: Option<String>,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    if !args.path.exists() {
        eprintln!("avplan: {}: no such file", args.path.display());
        return ExitCode::from(exit::IO);
    }
    for program in ["ffprobe", "ffmpeg"] {
        if which(program).is_none() {
            eprintln!("avplan: {program} is not on PATH");
            return ExitCode::from(exit::UNAVAILABLE);
        }
    }

    let path = args.path.display().to_string();
    let Some(video) = probe(&args.path, "v:0") else {
        eprintln!("avplan: ffprobe could not read {path}");
        return ExitCode::from(exit::INDETERMINATE);
    };
    let audio = probe(&args.path, "a:0").unwrap_or_default();

    let streams = Streams {
        video: Some(video).filter(|codec| !codec.is_empty()),
        audio: Some(audio).filter(|codec| !codec.is_empty()),
        container: avplan::container_of(&path),
    };

    let encoder = match args.encoder.as_deref() {
        None => installed(),
        Some(name) => match avplan::encoder::PREFERRED
            .iter()
            .find(|encoder| encoder.name() == name)
        {
            Some(encoder) => Some(*encoder),
            None => {
                eprintln!("avplan: {name:?} is not an encoder this knows");
                return ExitCode::from(exit::USAGE);
            }
        },
    };

    let output = args
        .output
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| avplan::output_beside(&path));
    let plan = avplan::plan(&path, &streams, encoder, &output, &args.vaapi_device);

    // The record form always writes, because the reasons are the answer even
    // when nothing can act on them. The command form has nothing to say
    // about a file that plays, and nothing to offer for one this machine
    // cannot convert.
    if args.json {
        let json = serde_json::to_string(&plan).expect("a plan serialises");
        return leave(catena_cli::text("avplan", &format!("{json}\n")), &plan);
    }
    if plan.playable {
        return ExitCode::from(exit::NO);
    }
    if plan.argv.is_empty() {
        eprintln!("avplan: no H.264 encoder on this machine, so nothing here can convert it");
        return ExitCode::from(exit::UNAVAILABLE);
    }
    leave(
        catena_cli::text("avplan", &format!("{}\n", quoted(&plan.argv))),
        &plan,
    )
}

/// Leave with the verdict, unless writing it failed.
fn leave(wrote: ExitCode, plan: &avplan::Plan) -> ExitCode {
    if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
        return wrote;
    }
    ExitCode::from(match (plan.playable, plan.argv.is_empty()) {
        (true, _) => exit::NO,
        (false, true) => exit::UNAVAILABLE,
        (false, false) => exit::OK,
    })
}

/// A command line a person can read and a shell can run.
fn quoted(argv: &[String]) -> String {
    std::iter::once("ffmpeg".to_string())
        .chain(argv.iter().cloned())
        .map(|arg| match arg.chars().all(is_bare) {
            true => arg,
            false => format!("'{}'", arg.replace('\'', r"'\''")),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_bare(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-_.:/=+,".contains(c)
}

/// The codec of one stream, or `None` if the file could not be read at all.
fn probe(path: &std::path::Path, stream: &str) -> Option<String> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            stream,
            "-show_entries",
            "stream=codec_name",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string(),
    )
}

/// The best encoder this machine has.
fn installed() -> Option<Encoder> {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .ok()?;
    avplan::encoder::best_available(&String::from_utf8_lossy(&out.stdout))
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| is_program(candidate))
}

#[cfg(unix)]
fn is_program(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_program(path: &std::path::Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_line_is_quoted_where_it_has_to_be() {
        let argv = vec!["-i".into(), "a file.webm".into(), "out.mp4".into()];
        assert_eq!(quoted(&argv), "ffmpeg -i 'a file.webm' out.mp4");
    }

    #[test]
    fn an_apostrophe_in_a_name_does_not_end_the_quoting() {
        let argv = vec!["-i".into(), "o'brien.webm".into()];
        assert_eq!(quoted(&argv), r"ffmpeg -i 'o'\''brien.webm'");
    }

    #[test]
    fn ordinary_arguments_are_left_bare() {
        let argv = vec!["-c:v".into(), "libx264".into(), "-crf".into(), "23".into()];
        assert_eq!(quoted(&argv), "ffmpeg -c:v libx264 -crf 23");
    }
}
