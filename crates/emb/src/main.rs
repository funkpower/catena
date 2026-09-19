//! Lines in, vectors out, against an HTTP embedding endpoint.

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use corpus_core::exit;
use emb::Endpoint;

/// Embed each line of the input and write its vector.
///
/// One JSON array per line, in the order the lines arrived, so the output
/// lines up with the input line for line. Empty lines are sent as they are:
/// dropping them would make the two disagree.
///
/// The only tool here that uses the network. It never loads a model — the
/// model belongs behind a server, where it can be shared and where its GPU
/// is somebody else's problem.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 every line was embedded, 65 the server answered something that
was not an embedding, 69 the server could not be reached, 74 the input could
not be read.

Example:

    jq -r .text chunks.ndjson | emb --model bge-m3 --dim 1024 > vectors.ndjson"
)]
struct Args {
    /// File of texts, one per line. Reads stdin when absent.
    path: Option<std::path::PathBuf>,

    /// The server to ask.
    #[arg(long, default_value = "http://localhost:11434", value_name = "URL")]
    host: String,

    /// The model to ask for.
    #[arg(long, value_name = "NAME")]
    model: String,

    /// Refuse vectors that are not this wide.
    #[arg(long, value_name = "WIDTH")]
    dim: Option<usize>,

    /// Texts per request.
    #[arg(long, default_value_t = emb::BATCH, value_name = "COUNT")]
    batch: usize,

    /// Cut each text to this many tokens before sending it. Zero sends them
    /// whole.
    #[arg(long, default_value_t = emb::MAX_INPUT_TOKENS, value_name = "TOKENS")]
    max_input_tokens: usize,

    /// How long to wait for one request.
    #[arg(long, default_value_t = emb::TIMEOUT.as_secs(), value_name = "SECONDS")]
    timeout: u64,

    /// How many times to try a failing request.
    #[arg(long, default_value_t = emb::ATTEMPTS, value_name = "COUNT")]
    attempts: u32,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };
    if args.batch == 0 {
        eprintln!("emb: --batch must be above zero");
        return ExitCode::from(exit::USAGE);
    }

    let endpoint = Endpoint {
        dim: args.dim,
        batch: args.batch,
        max_input_tokens: Some(args.max_input_tokens),
        timeout: Duration::from_secs(args.timeout),
        attempts: args.attempts,
        ..Endpoint::new(&args.host, &args.model)
    };

    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("emb: {why}");
            return ExitCode::from(code);
        }
    };
    // A trailing newline ends the last line rather than starting an empty one.
    let lines: Vec<String> = text
        .strip_suffix('\n')
        .unwrap_or(&text)
        .split('\n')
        .map(str::to_string)
        .collect();
    if text.is_empty() {
        return ExitCode::from(exit::OK);
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(endpoint.timeout))
        .build()
        .into();

    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for batch in lines.chunks(endpoint.batch) {
        let vectors = match emb::embed_batch(&agent, &endpoint, batch) {
            Ok(vectors) => vectors,
            Err(e) => {
                eprintln!("emb: {e}");
                return ExitCode::from(match e {
                    emb::Error::Http(_) => exit::UNAVAILABLE,
                    _ => exit::DATA,
                });
            }
        };
        for vector in vectors {
            let line = serde_json::to_string(&vector).expect("a vector serialises");
            match writeln!(out, "{line}").and_then(|()| out.flush()) {
                Ok(()) => {}
                // A consumer that stopped reading is not a failure.
                Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                    return ExitCode::from(exit::OK);
                }
                Err(e) => {
                    eprintln!("emb: {e}");
                    return ExitCode::from(exit::IO);
                }
            }
        }
    }
    catena_cli::finish("emb", out)
}
