//! What a catalogue says about an identifier.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use corpus_core::exit;

/// Ask the body that assigns an identifier what it knows about one.
///
/// `idnorm` can say a DOI is well formed. Only a register can say the paper
/// exists — that is a fact about the world rather than about the string, and
/// it is what catches a well-formed DOI a catalogue has been carrying wrongly
/// for three years.
///
/// Crossref for a DOI, the arXiv for a preprint, OpenLibrary for a book. This
/// is the only tool here besides `emb` that touches the network, and it is the
/// whole of what it does.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 every identifier was found, 1 at least one was not,
65 the input was not identifiers, 69 a register could not be reached,
74 the file could not be read.

Example:

    idnorm refs.txt | bibfetch | jq -r 'select(.found) | .title'
    idnorm --kind doi refs.txt | bibfetch --polite you@example.org
    bibfetch cover 978-0-306-40615-7 -o cover.jpg
    bibfetch search --title 'The First World War' --author Howard"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Identifier records to look up, as `idnorm` writes them. Reads stdin
    /// when absent.
    path: Option<PathBuf>,

    /// An address to put in the request, which the registers ask for and
    /// answer faster when they get.
    #[arg(long, global = true, value_name = "EMAIL")]
    polite: Option<String>,

    /// How long to wait on a register, in seconds.
    #[arg(long, global = true, default_value_t = bibfetch::TIMEOUT.as_secs(), value_name = "SECONDS")]
    timeout: u64,

    /// How many times to ask before believing a failure.
    #[arg(long, global = true, default_value_t = bibfetch::ATTEMPTS, value_name = "COUNT")]
    attempts: u32,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Fetch a book's cover from Open Library by its ISBN.
    ///
    /// Exit codes: 0 the cover was written, 1 Open Library has none,
    /// 65 not an ISBN or not an image, 69 Open Library could not be reached,
    /// 74 the file could not be written.
    Cover {
        /// The ISBN, ten or thirteen digits, hyphens or not.
        isbn: String,
        /// Where to write the image.
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        /// Which size to ask for.
        #[arg(long, value_enum, default_value_t = Size::Large)]
        size: Size,
        /// An Open Library cover id to ask by when the ISBN has no cover.
        #[arg(long, value_name = "ID")]
        cover_id: Option<u64>,
    },
    /// Find the books a register has under a title, and an author.
    ///
    /// Prints one record per candidate edition, in the order the register
    /// gave them: Open Library's index, or Google Books when that has
    /// nothing. Which one is the book in hand is left to you.
    ///
    /// Exit codes: 0 at least one candidate, 1 none, 65 no title,
    /// 69 no register could be reached, 74 the output could not be written.
    Search {
        /// The title, or as much of it as you have.
        #[arg(long)]
        title: String,
        /// An author's name, or part of one.
        #[arg(long)]
        author: Option<String>,
        /// The most candidates to print.
        #[arg(long, default_value_t = 10, value_name = "COUNT",
              value_parser = clap::value_parser!(u16).range(1..=bibfetch::search::MAX_LIMIT as i64))]
        limit: u16,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Size {
    Small,
    Medium,
    Large,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let agent = bibfetch::ask::Ask {
        polite: args.polite.clone(),
        timeout: Duration::from_secs(args.timeout),
        attempts: args.attempts,
    }
    .agent();

    match &args.command {
        Some(Command::Cover {
            isbn,
            output,
            size,
            cover_id,
        }) => return cover(&agent, args.attempts, isbn, output, *size, *cover_id),
        Some(Command::Search {
            title,
            author,
            limit,
        }) => {
            return search(
                &agent,
                args.attempts,
                title,
                author.as_deref(),
                usize::from(*limit),
            );
        }
        None => {}
    }

    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("bibfetch: {why}");
            return ExitCode::from(code);
        }
    };

    let mut records = Vec::new();
    let mut missing = 0usize;

    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (id, kind) = match read_identifier(line) {
            Ok(pair) => pair,
            Err(why) => {
                eprintln!("bibfetch: line {}: {why}", number + 1);
                return ExitCode::from(exit::DATA);
            }
        };

        match bibfetch::ask::lookup(&agent, &id, &kind, args.attempts) {
            Ok(None) => {
                eprintln!("bibfetch: {id}: no register here answers about {kind} identifiers");
                missing += 1;
            }
            Ok(Some(record)) => {
                if !record.found {
                    missing += 1;
                }
                records.push(record);
            }
            Err(why) => {
                eprintln!("bibfetch: {id}: {why}");
                return ExitCode::from(exit::UNAVAILABLE);
            }
        }
    }

    let wrote = catena_cli::stream("bibfetch", &records);
    if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
        return wrote;
    }
    ExitCode::from(match missing {
        0 => exit::OK,
        _ => exit::NO,
    })
}

/// Fetch one cover and write it where asked.
fn cover(
    agent: &ureq::Agent,
    attempts: u32,
    isbn: &str,
    output: &std::path::Path,
    size: Size,
    cover_id: Option<u64>,
) -> ExitCode {
    let covers = bibfetch::cover::Covers {
        attempts,
        ..Default::default()
    };
    let size = match size {
        Size::Small => bibfetch::CoverSize::Small,
        Size::Medium => bibfetch::CoverSize::Medium,
        Size::Large => bibfetch::CoverSize::Large,
    };
    match covers.fetch(agent, isbn, cover_id, size) {
        Ok(Some(found)) => match std::fs::write(output, &found.bytes) {
            Ok(()) => ExitCode::from(exit::OK),
            Err(why) => {
                eprintln!("bibfetch: {}: {why}", output.display());
                ExitCode::from(exit::IO)
            }
        },
        Ok(None) => {
            eprintln!("bibfetch: {isbn}: Open Library has no cover");
            ExitCode::from(exit::NO)
        }
        Err(why) => {
            eprintln!("bibfetch: {isbn}: {why}");
            ExitCode::from(match why {
                bibfetch::Error::Unreachable(_) => exit::UNAVAILABLE,
                _ => exit::DATA,
            })
        }
    }
}

/// Search by title and print the candidates.
fn search(
    agent: &ureq::Agent,
    attempts: u32,
    title: &str,
    author: Option<&str>,
    limit: usize,
) -> ExitCode {
    match bibfetch::search(agent, title, author, limit, attempts) {
        Ok(found) => {
            let wrote = catena_cli::stream("bibfetch", &found);
            if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
                return wrote;
            }
            if found.is_empty() {
                eprintln!("bibfetch: no register has {title:?}");
                return ExitCode::from(exit::NO);
            }
            ExitCode::from(exit::OK)
        }
        Err(why) => {
            eprintln!("bibfetch: {why}");
            ExitCode::from(match why {
                bibfetch::Error::Invalid(_) => exit::DATA,
                _ => exit::UNAVAILABLE,
            })
        }
    }
}

/// The identifier and its kind out of one record.
fn read_identifier(line: &str) -> Result<(String, String), String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("not a record: {e}"))?;
    let id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or("no id")?
        .to_string();
    let kind = value
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or("no kind")?
        .to_string();
    Ok((id, kind))
}
