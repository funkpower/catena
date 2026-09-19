//! The identifiers a bibliography is held together by, read and checked.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use corpus_core::exit;
use idnorm::Kind;

/// Read identifiers and say what they are, one record per line.
///
/// A DOI arrives bare, as a URL, with a `doi:` prefix, with the full stop the
/// sentence around it left behind, in whatever case the publisher felt like.
/// All of those are one identifier, and a catalogue storing them as they
/// arrived cannot tell.
///
/// Asks nobody whether the thing exists. What it can say is whether the
/// string is well formed — an ISBN and an ISSN carry a check digit — which is
/// a different question, and the honest answer to the other one needs a
/// catalogue.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 every line was an identifier, 1 at least one was not,
74 the file could not be read.

Example:

    idnorm refs.txt | jq -r 'select(.checksum == false) | .input'
    idnorm --kind doi refs.txt | jq -r .url"
)]
struct Args {
    /// File of identifiers, one per line. Reads stdin when absent.
    path: Option<PathBuf>,

    /// Read every line as this kind, rather than whatever it looks like.
    #[arg(long, value_enum)]
    kind: Option<Sort>,

    /// Say nothing about the lines that are not identifiers.
    #[arg(long)]
    quiet: bool,
}

/// The kinds a line can be insisted on being.
#[derive(Clone, Copy, clap::ValueEnum)]
enum Sort {
    /// A digital object identifier.
    Doi,
    /// A journal's serial number.
    Issn,
    /// A book's number.
    Isbn,
    /// An arXiv preprint.
    Arxiv,
}

impl From<Sort> for Kind {
    fn from(sort: Sort) -> Self {
        match sort {
            Sort::Doi => Kind::Doi,
            Sort::Issn => Kind::Issn,
            Sort::Isbn => Kind::Isbn,
            Sort::Arxiv => Kind::Arxiv,
        }
    }
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("idnorm: {why}");
            return ExitCode::from(code);
        }
    };

    let kind = args.kind.map(Kind::from);
    let mut found = Vec::new();
    let mut unread = 0usize;

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match idnorm::parse_as(line, kind) {
            Some(identifier) => found.push(identifier),
            None => {
                unread += 1;
                if !args.quiet {
                    eprintln!("idnorm: not an identifier: {line}");
                }
            }
        }
    }

    let wrote = catena_cli::stream("idnorm", &found);
    if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
        return wrote;
    }
    ExitCode::from(match unread {
        0 => exit::OK,
        _ => exit::NO,
    })
}
