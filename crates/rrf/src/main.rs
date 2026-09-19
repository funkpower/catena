//! Reciprocal rank fusion: several ranked lists in, one ranked list out.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::{Channel, Hit, exit, ndjson};
use rrf::Weights;

/// Fuse several ranked lists of hits into one.
///
/// Reads hit records on stdin, in any order and interleaved if you like —
/// each carries the rank its own channel gave it, so the stream does not have
/// to be grouped. Writes the fused list, best first, with `rank` and `score`
/// replaced by the fused ones.
///
/// Database-agnostic on purpose. By the time a hit reaches here it is already
/// yours, so this is the one tool in the suite that is worth calling in
/// process rather than as a subprocess.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Example:

    cat vector.ndjson fts.ndjson | rrf --limit 10

Weights are given as channel=number, and default to vec=1, fts=1, meta=1.5 —
a metadata match outranks a body-text match at the same rank, because a book
rarely repeats its own title in its body."
)]
struct Args {
    /// Hit stream to read. Reads stdin when absent.
    path: Option<PathBuf>,

    /// How many results to keep. All of them when absent.
    #[arg(long, short = 'n', value_name = "COUNT")]
    limit: Option<usize>,

    /// The rank constant.
    #[arg(long = "rrf-k", default_value_t = rrf::K, value_name = "K")]
    k: f64,

    /// What a channel's rank is worth, as `channel=number`. Repeatable.
    #[arg(long, value_name = "CHANNEL=WEIGHT")]
    weight: Vec<String>,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let weights = match weights(&args) {
        Ok(weights) => weights,
        Err(why) => {
            eprintln!("rrf: {why}");
            return ExitCode::from(exit::USAGE);
        }
    };

    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("rrf: {why}");
            return ExitCode::from(code);
        }
    };
    let hits = match ndjson::read(text.as_bytes()).collect::<Result<Vec<Hit>, _>>() {
        Ok(hits) => hits,
        Err(e) => {
            eprintln!("rrf: {e}");
            return ExitCode::from(if e.is_broken_pipe() {
                exit::OK
            } else {
                exit::DATA
            });
        }
    };

    let fused = rrf::fuse(&hits, &weights);
    let fused = &fused[..args.limit.unwrap_or(fused.len()).min(fused.len())];

    catena_cli::stream("rrf", fused)
}

fn weights(args: &Args) -> Result<Weights, String> {
    if !args.k.is_finite() || args.k <= 0.0 {
        return Err(format!("--rrf-k {} must be a positive number", args.k));
    }
    let mut weights = Weights {
        k: args.k,
        ..Weights::default()
    };
    for given in &args.weight {
        let (channel, weight) = given
            .split_once('=')
            .ok_or_else(|| format!("--weight {given:?} is not channel=number"))?;
        let channel: Channel = serde_json::from_value(serde_json::Value::String(channel.into()))
            .map_err(|_| format!("--weight {given:?} names no channel this knows"))?;
        let weight: f64 = weight
            .parse()
            .map_err(|_| format!("--weight {given:?} is not channel=number"))?;
        if !weight.is_finite() {
            return Err(format!("--weight {given:?} is not a number"));
        }
        weights.channels.insert(channel, weight);
    }
    Ok(weights)
}
