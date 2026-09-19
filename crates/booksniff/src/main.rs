//! Says what a file actually is, whatever its name claims.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use booksniff::bytes::File;
use clap::Parser;
use corpus_core::{Doc, Kind, exit, ndjson};

/// Identify files by what is in them rather than by what they are called.
///
/// Writes one doc record per file. Reads only the few kilobytes it needs —
/// the front of the file, and an archive's directory at the back — so a
/// directory of scans costs the same as a directory of anything else.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 every file was identified, 1 at least one was not, 74 a file
could not be read.

Example:

    booksniff library/*.pdf | jq -r 'select(.kind != \"pdf\") | .path'"
)]
struct Args {
    /// Files to identify.
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Print a line a person can read instead of the record.
    #[arg(long)]
    plain: bool,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut unknown = false;
    let mut unreadable = false;

    for path in &args.paths {
        let name = path.display().to_string();
        let doc = match File::open(path) {
            Ok(file) => booksniff::sniff(&file, &name),
            Err(e) => {
                eprintln!("booksniff: {name}: {e}");
                unreadable = true;
                continue;
            }
        };
        unknown |= doc.kind == Kind::Unknown;

        let wrote = if args.plain {
            plain(&mut out, &doc)
        } else {
            ndjson::write_line(&mut out, &doc).map_err(|e| match e {
                ndjson::Error::Io(e) => e,
                other => std::io::Error::other(other.to_string()),
            })
        };
        if let Err(e) = wrote {
            // A consumer that stopped reading is not a failure.
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                return ExitCode::from(exit::OK);
            }
            eprintln!("booksniff: {e}");
            return ExitCode::from(exit::IO);
        }
    }

    if let Err(e) = out.flush() {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            return ExitCode::from(exit::OK);
        }
        eprintln!("booksniff: {e}");
        return ExitCode::from(exit::IO);
    }

    ExitCode::from(match (unreadable, unknown) {
        (true, _) => exit::IO,
        (false, true) => exit::NO,
        (false, false) => exit::OK,
    })
}

fn plain(out: &mut impl Write, doc: &Doc) -> std::io::Result<()> {
    let kind = serde_json::to_string(&doc.kind).unwrap_or_default();
    writeln!(out, "{}\t{}", kind.trim_matches('"'), doc.path)?;
    for note in &doc.notes {
        writeln!(out, "\t{note}")?;
    }
    Ok(())
}
