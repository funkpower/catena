//! Finds books scanned two pages to a sheet, and splits them back into pages.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::Parser;
use corpus_core::exit;
use pdfgutter::{Page, Verdict};

/// Say whether a PDF is two facing pages photographed as one sheet, and split
/// it back into pages when it is.
///
/// A book held open under a camera gives one wide image per shot. Anything
/// that reads it afterwards runs the two columns together, and the text that
/// comes out looks like text while being unusable.
///
/// Shells out to `pdftoppm` to look at the pages. Splitting does not: it
/// changes the page boxes and leaves the content alone, so nothing is
/// re-rendered and nothing is lost.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 two pages to a sheet, 1 one page to a sheet, 2 could not tell,
69 pdftoppm is not installed, 74 the file could not be read or written.

Example:

    pdfgutter scan.pdf && pdfgutter --into split.pdf scan.pdf
    for f in *.pdf; do pdfgutter --quiet \"$f\" && pdfgutter --into \"split/$f\" \"$f\"; done"
)]
struct Args {
    /// The PDF to look at.
    path: PathBuf,

    /// Write the split document here. Implies nothing about the verdict: a
    /// document that is not two-up is copied through unchanged.
    #[arg(long, value_name = "FILE")]
    into: Option<PathBuf>,

    /// Emit a record instead of a sentence.
    #[arg(long)]
    json: bool,

    /// Say nothing; the exit code is the answer.
    #[arg(long, short)]
    quiet: bool,

    /// How wide a sheet has to be, as a multiple of its height, before it
    /// could hold two pages.
    #[arg(long, default_value_t = pdfgutter::MIN_RATIO, value_name = "RATIO")]
    min_ratio: f64,

    /// What share of the wide sheets looked at must show a gutter.
    #[arg(long, default_value_t = pdfgutter::MIN_GUTTER_FRACTION, value_name = "FRACTION")]
    gutter_fraction: f64,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };
    if !(args.min_ratio.is_finite() && args.gutter_fraction.is_finite()) {
        eprintln!("pdfgutter: a threshold has to be a number");
        return ExitCode::from(exit::USAGE);
    }

    let mut pdf = match pdfgutter::pdf::Pdf::open(&args.path) {
        Ok(pdf) => pdf,
        Err(why) => {
            eprintln!("pdfgutter: {}: {why}", args.path.display());
            return ExitCode::from(match why {
                pdfgutter::pdf::Error::Unreadable(_) => exit::IO,
                _ => exit::INDETERMINATE,
            });
        }
    };

    let verdict = match measure(&args, &pdf) {
        Ok(verdict) => verdict,
        Err(code) => return code,
    };

    // The verdict first, because it is the answer; what was written to disk
    // after it, because that is what happened.
    let said = say(&args, &verdict);

    if let Some(destination) = args.into.as_deref() {
        let pages = pdf.split(args.min_ratio);
        if let Err(why) = pdf.save(destination) {
            eprintln!("pdfgutter: {}: {why}", destination.display());
            return ExitCode::from(exit::IO);
        }
        if !args.quiet && !args.json {
            println!("{pages} pages written to {}", destination.display());
        }
    }

    said
}

/// Sample the document and decide.
fn measure(args: &Args, pdf: &pdfgutter::pdf::Pdf) -> Result<Verdict, ExitCode> {
    let mut rasteriser = None;
    let mut sampled = Vec::new();
    let mut ratios = Vec::new();
    let mut gutters = Vec::new();

    for index in pdfgutter::sample_indices(pdf.len()) {
        let ratio = pdf.geometry(index).ratio();
        ratios.push(ratio);

        // Rendering is the expensive part, and a sheet too narrow to hold two
        // pages cannot be two pages whatever its ink looks like.
        let gutter = match ratio > args.min_ratio {
            false => None,
            true => {
                if rasteriser.is_none() {
                    match which("pdftoppm") {
                        None => {
                            eprintln!("pdfgutter: pdftoppm is not on PATH");
                            return Err(ExitCode::from(exit::UNAVAILABLE));
                        }
                        Some(found) => rasteriser = Some(found),
                    }
                }
                Some(gutter_on(&args.path, index))
            }
        };
        if let Some(found) = gutter {
            gutters.push(found);
        }
        sampled.push(Page {
            number: index + 1,
            ratio,
            gutter,
        });
    }

    let median_ratio = pdfgutter::median(&ratios);
    let gutter_fraction = pdfgutter::gutter_fraction(&gutters);
    Ok(Verdict {
        v: corpus_core::V,
        path: args.path.display().to_string(),
        two_up: median_ratio > args.min_ratio && gutter_fraction >= args.gutter_fraction,
        pages: pdf.len(),
        median_ratio,
        gutter_fraction,
        sampled,
    })
}

/// Whether one page has a blank band down the middle with text either side.
///
/// A page that will not render is a page with no gutter rather than a reason
/// to give up on the document: one bad page in a book is ordinary.
fn gutter_on(path: &std::path::Path, index: usize) -> bool {
    let page = (index + 1).to_string();
    let rendered = Command::new("pdftoppm")
        .args([
            "-gray",
            "-scale-to-x",
            "1000",
            "-scale-to-y",
            "-1",
            "-singlefile",
        ])
        .args(["-f", &page, "-l", &page])
        .arg(path)
        .output();

    let Ok(rendered) = rendered else {
        return false;
    };
    if !rendered.status.success() {
        return false;
    }
    match pdfgutter::ink::profile(&rendered.stdout, pdfgutter::INK_THRESHOLD) {
        Ok(profile) => pdfgutter::has_gutter(&profile),
        Err(_) => false,
    }
}

/// Write the verdict, and leave with it.
fn say(args: &Args, verdict: &Verdict) -> ExitCode {
    let answer = match verdict.two_up {
        true => exit::OK,
        false => exit::NO,
    };
    if args.quiet {
        return ExitCode::from(answer);
    }

    let text = match args.json {
        true => format!(
            "{}\n",
            serde_json::to_string(verdict).expect("a verdict serialises")
        ),
        false => format!(
            "{}\n  {} pages, median ratio {:.2}, gutter on {:.0}% of the wide ones\n",
            match verdict.two_up {
                true => "two pages to a sheet",
                false => "one page to a sheet",
            },
            verdict.pages,
            verdict.median_ratio,
            verdict.gutter_fraction * 100.0
        ),
    };
    let wrote = catena_cli::text("pdfgutter", &text);
    match format!("{wrote:?}") == format!("{:?}", ExitCode::from(exit::OK)) {
        true => ExitCode::from(answer),
        false => wrote,
    }
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| is_program(candidate))
}

#[cfg(unix)]
fn is_program(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_program(path: &std::path::Path) -> bool {
    path.is_file()
}
