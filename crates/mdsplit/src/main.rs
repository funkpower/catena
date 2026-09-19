//! Splits one markdown file into the works inside it.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::exit;

/// Split a collection into the works inside it, one record per line.
///
/// A collected edition is one file holding forty books, and indexed whole a
/// query for any of them returns a passage that could be from any other. This
/// finds the seams.
///
/// Scripture is recognised and grouped by book rather than by chapter, since
/// every chapter there is its own heading and splitting on those would give a
/// thousand fragments instead of the books.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 it split, 1 it is not a collection, 65 the input was not text,
74 the file could not be read.

Example:

    mdsplit --into sections/ complete-works.md
    mdsplit complete-works.md | jq -r '\"\\(.idx)\\t\\(.title)\"'"
)]
struct Args {
    /// Markdown file to read. Reads stdin when absent.
    path: Option<PathBuf>,

    /// Write one file per section into this directory instead of writing
    /// records. Each is named for its position and its title.
    #[arg(long, value_name = "DIR")]
    into: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let markdown = match catena_cli::read_text(args.path.as_deref()) {
        Ok(markdown) => markdown,
        Err((code, why)) => {
            eprintln!("mdsplit: {why}");
            return ExitCode::from(code);
        }
    };

    let sections = match mdsplit::split(&markdown) {
        Ok(sections) => sections,
        Err(why) => {
            // Not a failure: "this is one work, not a collection" is an
            // answer, and the usual one.
            eprintln!("mdsplit: {why}");
            return ExitCode::from(exit::NO);
        }
    };

    match args.into.as_deref() {
        None => catena_cli::stream("mdsplit", &sections),
        Some(dir) => match write_files(dir, &sections) {
            Ok(()) => ExitCode::from(exit::OK),
            Err((code, why)) => {
                eprintln!("mdsplit: {why}");
                ExitCode::from(code)
            }
        },
    }
}

/// Write each section as its own file, named so a directory listing is in
/// reading order.
fn write_files(dir: &std::path::Path, sections: &[mdsplit::Section]) -> Result<(), (u8, String)> {
    std::fs::create_dir_all(dir).map_err(|e| (exit::IO, format!("{}: {e}", dir.display())))?;

    for section in sections {
        let path = dir.join(format!("{:03}-{}.md", section.idx, section.slug));
        std::fs::write(&path, &section.text)
            .map_err(|e| (exit::IO, format!("{}: {e}", path.display())))?;
        println!("{}", path.display());
    }
    Ok(())
}
