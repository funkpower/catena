//! Says what a file actually is, whatever its name claims.
//!
//! A library accumulates files that lie: a DjVu named `.pdf`, a Word document
//! named `.epub`, an EPUB saved as `.zip` and then never opened again, a
//! single PDF somebody zipped and renamed back to `.pdf`. Every one of those
//! fails somewhere downstream, usually as an unhelpful error from a converter
//! that was handed the wrong kind of file.
//!
//! The containers are unambiguous, so they are read rather than guessed at.

pub mod bytes;
pub mod mobi;
pub mod zip;

use corpus_core::{Doc, Kind};

use crate::bytes::Bytes;

/// DjVu's container magic: an IFF stream tagged for the format's originator.
const DJVU: &[u8] = b"AT&TFORM";
/// The OLE2 compound-file header every binary Word document starts with.
const OLE2: &[u8] = &[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
/// A PDF announces its version in its first bytes.
const PDF: &[u8] = b"%PDF-";

/// Archive members that carry no content: resource forks and desktop clutter.
const ARCHIVE_NOISE: &[&str] = &["__macosx/", ".ds_store", "thumbs.db"];

/// Substrings in a name that mark a file as a sample rather than a book.
const SAMPLE_MARKERS: &[&str] = &["sample", "readme", "read me"];

/// Below this, a file is too small to be a book whatever it claims to be.
const SMALLEST_BOOK: u64 = 4096;

/// How much of the front is read to tell text from markup.
const TEXT_WINDOW: usize = 4096;

/// Identify a file.
///
/// `name` is used only for the extension it claims and for the words in it,
/// both of which are reported as observations rather than believed.
pub fn sniff(source: &impl Bytes, name: &str) -> Doc {
    let mut doc = Doc::new(name, Kind::Unknown, source.len() as i64);
    doc.declared_ext = extension(name);

    doc.kind = if source.magic_at(0, DJVU) {
        Kind::Djvu
    } else if source.magic_at(0, PDF) {
        Kind::Pdf
    } else if source.magic_at(0, OLE2) {
        Kind::Ole2
    } else if source.magic_at(0, zip::LOCAL_HEADER) {
        archive(source, &mut doc)
    } else if let Some(kind) = mobi::kind(source) {
        kind
    } else {
        text(source, &mut doc)
    };

    if doc.kind == Kind::Unknown {
        doc.note("no container or magic bytes this recognises");
    }
    if let Some(declared) = doc.declared_ext.as_deref()
        && !names_the_same_thing(declared, doc.kind)
    {
        doc.note(format!(
            "named .{declared}, but the bytes say {}",
            spelling(doc.kind)
        ));
    }
    if source.len() < SMALLEST_BOOK {
        doc.note(format!(
            "{} bytes, which is too small to be a book",
            source.len()
        ));
    }
    if let Some(marker) = sample_marker(name) {
        doc.note(format!("the name says {marker:?}, so it may be a sample"));
    }
    doc
}

/// What is inside a zip: an EPUB, a wrapped PDF, or a collection of pages.
fn archive(source: &impl Bytes, doc: &mut Doc) -> Kind {
    let Some(names) = zip::members(source) else {
        doc.note("a zip whose directory could not be read");
        return Kind::Unknown;
    };
    let lower: Vec<String> = names.iter().map(|name| name.to_lowercase()).collect();

    if lower.iter().any(|name| name == "mimetype")
        && lower
            .iter()
            .any(|name| name.starts_with("meta-inf/container.xml"))
    {
        return Kind::Epub;
    }
    if let Some(pdf) = sole_pdf(&names) {
        doc.note(format!("a zip holding one PDF, {pdf:?}"));
        return Kind::ZipWrappedPdf;
    }
    if lower
        .iter()
        .any(|name| name.ends_with(".htm") || name.ends_with(".html") || name.ends_with(".xhtml"))
    {
        doc.note("a zip of HTML documents");
        return Kind::Unknown;
    }
    doc.note(format!(
        "a zip of {} members, none of them a book",
        names.len()
    ));
    Kind::Unknown
}

/// The one PDF an archive exists to carry, if that is what it is.
///
/// Only an unambiguous wrapper qualifies. An archive holding several PDFs is
/// a collection, and choosing among them would be a guess — better to say it
/// is not a book than to name one chapter as the whole.
pub fn sole_pdf(names: &[String]) -> Option<&str> {
    let real: Vec<&String> = names
        .iter()
        .filter(|name| !name.ends_with('/'))
        .filter(|name| {
            let lower = name.to_lowercase();
            !ARCHIVE_NOISE.iter().any(|noise| lower.contains(noise))
        })
        .collect();
    match real.as_slice() {
        [only] if only.to_lowercase().ends_with(".pdf") => Some(only.as_str()),
        _ => None,
    }
}

/// Text, markup, or nothing this knows.
fn text(source: &impl Bytes, doc: &mut Doc) -> Kind {
    let Ok(head) = source.slice(0, TEXT_WINDOW) else {
        return Kind::Unknown;
    };
    if head.is_empty() {
        doc.note("the file is empty");
        return Kind::Unknown;
    }
    // A file with a null byte in its first few kilobytes is not text, and
    // nothing else here is going to identify it either.
    if head.contains(&0) {
        return Kind::Unknown;
    }
    // The window may have cut a character in half, which is not the same
    // thing as the file not being text.
    let head = match std::str::from_utf8(&head) {
        Ok(text) => text,
        Err(e) if e.error_len().is_none() => {
            std::str::from_utf8(&head[..e.valid_up_to()]).expect("a valid prefix")
        }
        Err(_) => return Kind::Unknown,
    };

    let start = head
        .trim_start_matches(|c: char| textkit::is_space(c) || c == '\u{feff}')
        .to_lowercase();
    if start.starts_with("<!doctype html")
        || start.starts_with("<html")
        || start.starts_with("<?xml")
        || start.starts_with("<head")
    {
        return Kind::Html;
    }
    Kind::Txt
}

/// Whether an extension and a verdict are saying the same thing.
fn names_the_same_thing(extension: &str, kind: Kind) -> bool {
    match kind {
        Kind::Pdf => extension == "pdf",
        Kind::Epub => extension == "epub",
        Kind::Djvu => matches!(extension, "djvu" | "djv"),
        Kind::Azw3 => matches!(extension, "azw3" | "azw" | "kfx"),
        Kind::Mobi => matches!(extension, "mobi" | "prc" | "azw"),
        Kind::Txt => matches!(extension, "txt" | "text" | "md" | "markdown"),
        Kind::Html => matches!(extension, "html" | "htm" | "xhtml" | "xml"),
        Kind::Ole2 => matches!(extension, "doc" | "xls" | "ppt"),
        Kind::ZipWrappedPdf => extension == "zip",
        // Nothing was recognised, so there is nothing to disagree with.
        Kind::Unknown => true,
    }
}

fn spelling(kind: Kind) -> &'static str {
    match kind {
        Kind::Pdf => "PDF",
        Kind::Epub => "EPUB",
        Kind::Djvu => "DjVu",
        Kind::Azw3 => "AZW3",
        Kind::Mobi => "MOBI",
        Kind::Txt => "plain text",
        Kind::Html => "HTML",
        Kind::Ole2 => "an OLE2 document",
        Kind::ZipWrappedPdf => "a zip holding one PDF",
        Kind::Unknown => "nothing recognisable",
    }
}

fn extension(name: &str) -> Option<String> {
    std::path::Path::new(name)
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
}

fn sample_marker(name: &str) -> Option<&'static str> {
    let lower = std::path::Path::new(name)
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    SAMPLE_MARKERS
        .iter()
        .find(|marker| lower.contains(**marker))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn padded(head: &[u8]) -> Vec<u8> {
        let mut out = head.to_vec();
        out.resize(SMALLEST_BOOK as usize + 1, b'x');
        out
    }

    fn kind_of(head: &[u8], name: &str) -> Kind {
        sniff(&padded(head).as_slice(), name).kind
    }

    #[test]
    fn the_magic_decides_not_the_name() {
        assert_eq!(kind_of(b"%PDF-1.7\n", "book.epub"), Kind::Pdf);
        assert_eq!(kind_of(DJVU, "scan.pdf"), Kind::Djvu);
        assert_eq!(kind_of(OLE2, "book.epub"), Kind::Ole2);
    }

    #[test]
    fn a_name_that_disagrees_with_the_bytes_is_noted() {
        let doc = sniff(&padded(DJVU).as_slice(), "scan.pdf");
        assert_eq!(doc.kind, Kind::Djvu);
        assert!(
            doc.notes.iter().any(|note| note.contains("named .pdf")),
            "{:?}",
            doc.notes
        );
    }

    #[test]
    fn a_name_that_agrees_is_not_remarked_on() {
        let doc = sniff(&padded(b"%PDF-1.7").as_slice(), "book.pdf");
        assert!(doc.notes.is_empty(), "{:?}", doc.notes);
    }

    #[test]
    fn text_and_markup_are_told_apart() {
        assert_eq!(
            kind_of(b"The mill stood at the northern end.", "a.txt"),
            Kind::Txt
        );
        assert_eq!(kind_of(b"<!DOCTYPE html>\n<html>", "a.html"), Kind::Html);
        assert_eq!(kind_of(b"\n\n  <html lang=\"en\">", "a.txt"), Kind::Html);
        assert_eq!(kind_of(b"<?xml version=\"1.0\"?>", "a.xml"), Kind::Html);
    }

    #[test]
    fn binary_rubbish_is_not_called_text() {
        assert_eq!(kind_of(&[0x00, 0x01, 0x02, 0x03], "a.txt"), Kind::Unknown);
        assert_eq!(kind_of(&[0xff, 0xfe, 0xfd], "a.txt"), Kind::Unknown);
    }

    #[test]
    fn a_small_file_is_remarked_on() {
        let doc = sniff(&b"%PDF-1.7".as_slice(), "tiny.pdf");
        assert!(
            doc.notes.iter().any(|note| note.contains("too small")),
            "{:?}",
            doc.notes
        );
    }

    #[test]
    fn a_name_that_says_sample_is_remarked_on() {
        let doc = sniff(&padded(b"%PDF-1.7").as_slice(), "Book - Sample Chapter.pdf");
        assert!(
            doc.notes.iter().any(|note| note.contains("sample")),
            "{:?}",
            doc.notes
        );
    }

    #[test]
    fn the_one_pdf_in_an_archive_is_the_book_and_several_are_not() {
        let one = ["__MACOSX/._x".into(), "dir/".into(), "book.pdf".into()];
        assert_eq!(sole_pdf(&one), Some("book.pdf"));

        let several = ["one.pdf".to_string(), "two.pdf".to_string()];
        assert_eq!(sole_pdf(&several), None);

        let mixed = ["book.pdf".to_string(), "cover.jpg".to_string()];
        assert_eq!(sole_pdf(&mixed), None);
    }

    #[test]
    fn an_empty_file_is_unknown_and_says_so() {
        let doc = sniff(&b"".as_slice(), "nothing.pdf");
        assert_eq!(doc.kind, Kind::Unknown);
        assert!(doc.notes.iter().any(|note| note.contains("empty")));
        assert_eq!(doc.bytes, 0);
    }

    #[test]
    fn the_declared_extension_is_reported_lowercased() {
        assert_eq!(
            sniff(&padded(b"%PDF-").as_slice(), "BOOK.PDF").declared_ext,
            Some("pdf".to_string())
        );
        assert_eq!(
            sniff(&padded(b"%PDF-").as_slice(), "no-extension").declared_ext,
            None
        );
    }

    #[test]
    fn a_character_cut_in_half_by_the_window_does_not_make_it_binary() {
        let mut head = "é".repeat(TEXT_WINDOW).into_bytes();
        head.truncate(TEXT_WINDOW + 1);
        assert_eq!(sniff(&head.as_slice(), "a.txt").kind, Kind::Txt);
    }
}
