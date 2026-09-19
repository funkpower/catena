//! Prints the JSON Schema for the record types, so a consumer in another
//! language can generate its own bindings instead of reading Rust.

use std::process::ExitCode;

use corpus_core::{Chunk, Doc, Hit, exit};

const USAGE: &str = "\
usage: corpus-core schema [doc|chunk|hit|all]

Prints the JSON Schema for one record type, or an object of all three.

    cargo run -p corpus-core -- schema chunk > chunk.schema.json
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("schema") => {
            let which = args.next().unwrap_or_else(|| "all".into());
            match schema(&which) {
                Some(json) => {
                    println!("{json}");
                    ExitCode::from(exit::OK as u8)
                }
                None => {
                    eprintln!("corpus-core: unknown record type {which:?}\n\n{USAGE}");
                    ExitCode::from(exit::USAGE as u8)
                }
            }
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            ExitCode::from(exit::OK as u8)
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(exit::USAGE as u8)
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
