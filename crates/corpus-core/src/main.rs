//! Prints the JSON Schema for the record types, so a consumer in another
//! language can generate its own bindings instead of reading Rust.

use std::io::Write;
use std::process::ExitCode;

use corpus_core::{Chunk, Doc, Hit, exit};

const USAGE: &str = "\
usage: corpus-core schema [doc|chunk|hit|all]

Prints the JSON Schema for one record type, or an object of all three.

    cargo run -p corpus-core -- schema chunk > chunk.schema.json
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match args.as_slice() {
        ["--help" | "-h"] => print(USAGE),
        ["--version"] => print(&format!("corpus-core {}\n", env!("CARGO_PKG_VERSION"))),
        ["schema"] => emit("all"),
        ["schema", which] => emit(which),
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(exit::USAGE)
        }
    }
}

fn emit(which: &str) -> ExitCode {
    match schema(which) {
        Some(json) => print(&format!("{json}\n")),
        None => {
            eprintln!("corpus-core: unknown record type {which:?}\n");
            eprint!("{USAGE}");
            ExitCode::from(exit::USAGE)
        }
    }
}

/// Write to stdout, treating a consumer that stopped reading as the end of
/// the job rather than as a failure. `println!` would panic there.
fn print(text: &str) -> ExitCode {
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(()) => ExitCode::from(exit::OK),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::from(exit::OK),
        Err(e) => {
            eprintln!("corpus-core: {e}");
            ExitCode::from(exit::IO)
        }
    }
}

fn schema(which: &str) -> Option<String> {
    let json = match which {
        "doc" => serde_json::to_string_pretty(&schemars::schema_for!(Doc)),
        "chunk" => serde_json::to_string_pretty(&schemars::schema_for!(Chunk)),
        "hit" => serde_json::to_string_pretty(&schemars::schema_for!(Hit)),
        "all" => serde_json::to_string_pretty(&serde_json::json!({
            "doc": schemars::schema_for!(Doc),
            "chunk": schemars::schema_for!(Chunk),
            "hit": schemars::schema_for!(Hit),
        })),
        _ => return None,
    };
    Some(json.expect("a schema is always serialisable"))
}
