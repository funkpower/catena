//! Cuts markdown into retrievable passages that carry their page, chapter
//! and role.

use std::path::PathBuf;
use std::process::ExitCode;

use chunkmd::Budgets;
use clap::Parser;
use corpus_core::exit;

/// Cut markdown into chunks, one JSON record per line.
///
/// Reads markdown on stdin, or from a path. Page markers (`<!-- Page 41 -->`)
/// and headings become the page and chapter on every chunk, so a passage can
/// always be cited back to where it came from.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Example:

    mdscrub book.md | chunkmd --target 1024 --overlap 128 > chunks.ndjson

The defaults suit a model with a 512-token window; a larger model wants
--target 1024 --overlap 128 --min-tokens 80 --hard-max 1280."
)]
struct Args {
    /// Markdown file to read. Reads stdin when absent.
    path: Option<PathBuf>,

    /// Flush a chunk at this many tokens.
    #[arg(long, default_value_t = Budgets::default().target, value_name = "TOKENS")]
    target: usize,

    /// Carry this much of each chunk into the next one.
    #[arg(long, default_value_t = Budgets::default().overlap, value_name = "TOKENS")]
    overlap: usize,

    /// Drop chunks smaller than this.
    #[arg(long, default_value_t = Budgets::default().min_tokens, value_name = "TOKENS")]
    min_tokens: usize,

    /// Never emit a chunk larger than this.
    #[arg(long, default_value_t = Budgets::default().hard_max, value_name = "TOKENS")]
    hard_max: usize,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };
    let budgets = Budgets {
        target: args.target,
        overlap: args.overlap,
        min_tokens: args.min_tokens,
        hard_max: args.hard_max,
    };
    if let Err(why) = sane(&budgets) {
        eprintln!("chunkmd: {why}");
        return ExitCode::from(exit::USAGE);
    }

    let markdown = match catena_cli::read_text(args.path.as_deref()) {
        Ok(markdown) => markdown,
        Err((code, why)) => {
            eprintln!("chunkmd: {why}");
            return ExitCode::from(code);
        }
    };

    catena_cli::stream("chunkmd", &chunkmd::chunk(&markdown, &budgets))
}

/// Budgets that cannot produce anything sensible, rejected before reading
/// input rather than after.
fn sane(budgets: &Budgets) -> Result<(), String> {
    if budgets.target == 0 || budgets.hard_max == 0 {
        return Err("--target and --hard-max must be above zero".into());
    }
    if budgets.overlap >= budgets.target {
        return Err(format!(
            "--overlap {} must be below --target {}, or a chunk is all overlap",
            budgets.overlap, budgets.target
        ));
    }
    if budgets.hard_max < budgets.target {
        return Err(format!(
            "--hard-max {} must be at least --target {}",
            budgets.hard_max, budgets.target
        ));
    }
    if budgets.min_tokens > budgets.hard_max {
        return Err(format!(
            "--min-tokens {} is above --hard-max {}, so every chunk would be dropped",
            budgets.min_tokens, budgets.hard_max
        ));
    }
    Ok(())
}
