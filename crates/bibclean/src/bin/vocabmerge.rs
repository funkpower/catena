//! Finds the near-duplicates in a catalogue's vocabulary.

use std::path::PathBuf;
use std::process::ExitCode;

use bibclean::vocab::{Item, Kind, Rules};
use clap::Parser;
use corpus_core::{exit, ndjson};

/// Read a list of catalogue headings and write what should happen to them.
///
/// One JSON object per line — an `id`, a `name`, and the `count` of works
/// filed under it — and one proposal per line out: which heading to keep,
/// which to lose, by what rule, and how far to trust it.
///
/// Nothing is merged. Two headings that normalise to the same string are
/// usually the same thing and occasionally are not, and a catalogue that has
/// merged two real people is harder to repair than one that never did.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 nothing to propose, 1 something to propose, 65 a heading could
not be read, 74 the input could not be read.

Example:

    vocabmerge --kind author < authors.ndjson | jq -r '\"\\(.drop)\\t→\\t\\(.keep)\"'"
)]
struct Args {
    /// Heading list to read. Reads stdin when absent.
    path: Option<PathBuf>,

    /// What kind of heading the list holds.
    #[arg(long, value_enum)]
    kind: Heading,

    /// A JSON file of vocabulary specific to this collection.
    ///
    /// `{"device_prefixes": ["…"]}` — the names of e-readers that stamp
    /// themselves onto every book they touch. The generic ones are built in;
    /// a device named after its owner is a fact about one collection and
    /// belongs here.
    #[arg(long, value_name = "FILE")]
    rules: Option<PathBuf>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Heading {
    /// A person who wrote something.
    Author,
    /// A publisher or an imprint.
    Publisher,
    /// A subject heading.
    Tag,
}

impl From<Heading> for Kind {
    fn from(heading: Heading) -> Self {
        match heading {
            Heading::Author => Kind::Author,
            Heading::Publisher => Kind::Publisher,
            Heading::Tag => Kind::Tag,
        }
    }
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let rules = match args.rules.as_deref() {
        None => Rules::default(),
        Some(path) => match read_rules(path) {
            Ok(rules) => rules,
            Err((code, why)) => {
                eprintln!("vocabmerge: {why}");
                return ExitCode::from(code);
            }
        },
    };

    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("vocabmerge: {why}");
            return ExitCode::from(code);
        }
    };
    let items: Vec<Item> = match ndjson::read_any(text.as_bytes()).collect() {
        Ok(items) => items,
        Err(e) => {
            eprintln!("vocabmerge: {e}");
            return ExitCode::from(exit::DATA);
        }
    };

    let proposals = bibclean::vocab::cluster(args.kind.into(), &items, &rules);
    let anything = !proposals.is_empty();

    let wrote = catena_cli::stream("vocabmerge", &proposals);
    if format!("{wrote:?}") != format!("{:?}", ExitCode::from(exit::OK)) {
        return wrote;
    }
    ExitCode::from(if anything { exit::NO } else { exit::OK })
}

fn read_rules(path: &std::path::Path) -> Result<Rules, (u8, String)> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| (exit::IO, format!("{}: {e}", path.display())))?;
    serde_json::from_str(&text).map_err(|e| (exit::DATA, format!("{}: {e}", path.display())))
}
