//! Says whether a PDF's text layer is worth reading, or whether it has to be
//! OCRed.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::Parser;
use corpus_core::{Doc, Kind, TextLayer, exit};
use pdftriage::{Limits, Sample};

/// Pages sampled from the middle of the document.
const SPAN: u32 = 5;

/// Judge a PDF's text layer: healthy, garbled, sparse, reflected-font, or
/// absent — and so whether OCR is worth its cost.
///
/// Takes a path rather than a stream, because it reads two specific page
/// ranges rather than the whole file.
///
/// Shells out to `pdftotext`, and to `pdfinfo` for the page count; both ship
/// with poppler. Without `pdftotext` there is nothing to judge and the tool
/// exits 69. It never runs OCR, never writes to the file, and imposes no
/// timeout of its own — a caller that needs one owns it, which is half the
/// reason this is a separate process.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "\
Exit codes: 0 the layer needs OCR, 1 it does not, 2 could not tell,
69 pdftotext is not installed, 74 the file could not be read.

Example:

    for f in *.pdf; do pdftriage --quiet \"$f\" && ocrmypdf \"$f\" \"ocr/$f\"; done"
)]
struct Args {
    /// The PDF to judge.
    path: PathBuf,

    /// Emit a doc record instead of a human-readable verdict.
    #[arg(long)]
    json: bool,

    /// Write no verdict; answer with the exit code alone. Diagnostics still go
    /// to stderr, because a silent failure is worse than a quiet one.
    #[arg(long, short)]
    quiet: bool,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };

    let bytes = match std::fs::metadata(&args.path) {
        Ok(meta) => meta.len() as i64,
        Err(e) => {
            eprintln!("pdftriage: {}: {e}", args.path.display());
            return ExitCode::from(exit::IO);
        }
    };

    for program in ["pdftotext", "pdfinfo"] {
        if which(program).is_none() {
            eprintln!("pdftriage: {program} is not on PATH (it ships with poppler)");
            return ExitCode::from(exit::UNAVAILABLE);
        }
    }

    // Both of them, because without the page count the sample window falls
    // back to the front of the book and the density gate cannot run at all —
    // and a file would then be judged differently on two machines that both
    // have everything the tool said it needed.
    let pages = page_count(&args.path);
    let (first, last) = pdftriage::middle_window(pages, SPAN);

    let opening = extract(&args.path, 1, SPAN);
    let middle = extract(&args.path, first, last);
    let (Some(opening), Some(middle)) = (opening, middle) else {
        eprintln!(
            "pdftriage: pdftotext could not read {}",
            args.path.display()
        );
        if args.json && !args.quiet {
            let mut doc = unjudged(&args.path, bytes, pages);
            doc.note("pdftotext could not read the file");
            let _ = emit_json(&doc);
        }
        return ExitCode::from(exit::INDETERMINATE);
    };

    let found = pdftriage::triage(
        &Sample {
            opening: &opening,
            middle: &middle,
            middle_pages: last.saturating_sub(first) + 1,
            pages,
        },
        &Limits::default(),
    );

    if !args.quiet {
        let wrote = if args.json {
            let mut doc = unjudged(&args.path, bytes, pages);
            doc.text_layer = Some(found.layer);
            doc.needs_ocr = Some(found.needs_ocr);
            doc.notes = found.notes.clone();
            emit_json(&doc)
        } else {
            emit_report(&found)
        };
        if let Err(e) = wrote {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                return ExitCode::from(exit::OK);
            }
            eprintln!("pdftriage: {e}");
            return ExitCode::from(exit::IO);
        }
    }

    ExitCode::from(if found.needs_ocr { exit::OK } else { exit::NO })
}

fn unjudged(path: &Path, bytes: i64, pages: Option<u32>) -> Doc {
    let mut doc = Doc::new(path.display().to_string(), Kind::Pdf, bytes);
    doc.declared_ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase());
    doc.pages = pages;
    doc
}

fn emit_json(doc: &Doc) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    let json = serde_json::to_string(doc).expect("a doc record serialises");
    writeln!(out, "{json}")?;
    out.flush()
}

fn emit_report(found: &pdftriage::Triage) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    let verdict = match found.layer {
        TextLayer::Healthy => "healthy",
        TextLayer::Garbled => "garbled",
        TextLayer::Sparse => "sparse",
        TextLayer::ReflectedFont => "reflected-font",
        TextLayer::Absent => "absent",
    };
    let advice = if found.needs_ocr {
        "needs OCR"
    } else {
        "no OCR needed"
    };
    writeln!(out, "{verdict}, {advice}")?;
    for note in &found.notes {
        writeln!(out, "  {note}")?;
    }
    out.flush()
}

/// Extract a page range as text, or `None` when the extractor could not.
fn extract(path: &Path, first: u32, last: u32) -> Option<String> {
    let out = Command::new("pdftotext")
        .args(["-f", &first.to_string(), "-l", &last.to_string(), "-layout"])
        .arg(path)
        .arg("-")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| pdftriage::decode::decode(&out.stdout))
}

/// The document's page count, when `pdfinfo` can say.
fn page_count(path: &Path) -> Option<u32> {
    let out = Command::new("pdfinfo").arg(path).output().ok()?;
    if !out.status.success() {
        return None;
    }
    pdftriage::decode::decode(&out.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Pages:"))
        .and_then(|count| count.trim().parse().ok())
}

/// Find a program on `PATH`, the way a shell would.
///
/// Checking that a file exists is not enough: a non-executable file of the
/// right name shadows the real program and turns a clear "not installed" into
/// an obscure failure several steps later. On Windows the name on `PATH` also
/// carries an extension, which is what `PATHEXT` lists.
fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let extensions: Vec<String> = match std::env::var("PATHEXT") {
        Ok(list) => std::iter::once(String::new())
            .chain(list.split(';').map(str::to_lowercase))
            .collect(),
        Err(_) => vec![String::new()],
    };

    std::env::split_paths(&path)
        .flat_map(|dir| {
            extensions
                .iter()
                .map(move |extension| dir.join(format!("{program}{extension}")))
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_that_exists_is_found_and_one_that_does_not_is_not() {
        assert!(which("sh").is_some(), "every system has a shell");
        assert!(which("definitely-not-a-program-93a1").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_is_not_executable_is_not_a_program() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = std::env::temp_dir().join("pdftriage-which");
        std::fs::create_dir_all(&dir).expect("a writable directory");
        let path = dir.join("not-a-program-8813");
        std::fs::write(&path, b"#!/bin/sh\n").expect("a writable path");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("permissions to set");

        assert!(!is_program(&path));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("permissions to set");
        assert!(is_program(&path));
    }
}
