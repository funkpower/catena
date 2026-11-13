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
pub mod fb2;
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
/// Microsoft Reader's signature, ahead of everything else in the file.
const LIT: &[u8] = b"ITOLITLS";
/// The group every RTF document is, opened with the control word that names
/// the format.
const RTF: &[u8] = br"{\rtf";
/// The UTF-8 byte-order mark.
const BOM: &[u8] = b"\xef\xbb\xbf";

/// Archive members that carry no content: resource forks and desktop clutter.
const ARCHIVE_NOISE: &[&str] = &["__macosx/", ".ds_store", "thumbs.db"];

/// What a comic book archive carries besides its pages: the catalogue entry
/// comic readers write into it.
const COMIC_CATALOGUE: &str = "comicinfo.xml";

/// The extensions that say an archive is a comic book.
const COMIC_EXTENSIONS: &[&str] = &["cbz", "cbr"];

/// The extensions of the images a comic book archive is made of.
const PAGE_IMAGES: &[&str] = &[".jpg", ".jpeg", ".png", ".gif", ".webp"];

/// Substrings in a name that mark a file as a sample rather than a book.
const SAMPLE_MARKERS: &[&str] = &["sample", "readme", "read me"];

/// Below this, a file is too small to be a book whatever it claims to be.
const SMALLEST_BOOK: u64 = 4096;

/// How much of the front is read to tell RTF, FictionBook, markup and text
/// apart.
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
    } else if source.magic_at(0, LIT) {
        Kind::Lit
    } else if source.magic_at(0, zip::LOCAL_HEADER) {
        archive(source, &mut doc)
    } else if let Some(kind) = mobi::kind(source) {
        kind
    } else {
        match source.slice(0, TEXT_WINDOW) {
            Ok(head) => front(&head, &mut doc),
            Err(_) => Kind::Unknown,
        }
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

/// What is inside a zip: an EPUB, a wrapped book, a comic, or a collection of
/// pages.
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
    if let Some(fb2) = sole_member(&names, ".fb2") {
        doc.note(format!("a zip holding one FictionBook, {fb2:?}"));
        return Kind::ZipWrappedFb2;
    }
    if let Some(pages) = comic_pages(&names) {
        // A zip of pictures is as often a holiday as a comic, and only the
        // names have been read. It is a comic when something says so: the
        // catalogue entry a comic reader writes, or a name that claims it.
        let named_comic = doc
            .declared_ext
            .as_deref()
            .is_some_and(|ext| COMIC_EXTENSIONS.contains(&ext));
        if pages.catalogued || named_comic {
            return Kind::Cbz;
        }
        doc.note(format!(
            "a zip of {} image{}, which would be a comic book if its name said .cbz",
            pages.count,
            if pages.count == 1 { "" } else { "s" }
        ));
        return Kind::Unknown;
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
    sole_member(names, ".pdf")
}

/// The one member an archive exists to carry, if it is the only real member
/// and its name ends in `extension`.
fn sole_member<'a>(names: &'a [String], extension: &str) -> Option<&'a str> {
    match real_members(names).as_slice() {
        [only] if only.to_lowercase().ends_with(extension) => Some(only.as_str()),
        _ => None,
    }
}

/// The pages of an archive that is images and nothing else.
struct Pages {
    /// How many images there are.
    count: usize,
    /// Whether a comic reader's catalogue entry is there beside them.
    catalogued: bool,
}

/// The pages, if an archive is images and nothing else.
///
/// Every real member has to be an image. One PDF or one text file among the
/// pages makes it something else, and saying what would be a guess. Images
/// are known by their names alone: only the archive's directory is read.
fn comic_pages(names: &[String]) -> Option<Pages> {
    let mut catalogued = false;
    let pages: Vec<String> = real_members(names)
        .into_iter()
        .map(|name| name.to_lowercase())
        .filter(|name| {
            let file = name.rsplit('/').next().unwrap_or(name);
            let catalogue = file == COMIC_CATALOGUE;
            catalogued |= catalogue;
            !catalogue
        })
        .collect();
    let all_images = !pages.is_empty()
        && pages
            .iter()
            .all(|name| PAGE_IMAGES.iter().any(|image| name.ends_with(image)));
    all_images.then_some(Pages {
        count: pages.len(),
        catalogued,
    })
}

/// The members that carry content: no directories, no desktop clutter.
fn real_members(names: &[String]) -> Vec<&String> {
    names
        .iter()
        .filter(|name| !name.ends_with('/'))
        .filter(|name| {
            let lower = name.to_lowercase();
            !ARCHIVE_NOISE.iter().any(|noise| lower.contains(noise))
        })
        .collect()
}

/// What the front of a file says when it is not a container: RTF, a
/// FictionBook, markup, text, or nothing this knows.
fn front(head: &[u8], doc: &mut Doc) -> Kind {
    // Both are recognised on bytes rather than text. RTF is seven-bit by
    // design but not always in practice, and a FictionBook is as often
    // windows-1251 as UTF-8.
    let lead = head.strip_prefix(BOM).unwrap_or(head).trim_ascii_start();
    if is_rtf(lead) {
        return Kind::Rtf;
    }
    if fb2::is_fictionbook(head) {
        return Kind::Fb2;
    }
    text(head, doc)
}

/// Whether `lead` opens an RTF document.
///
/// `{\rtf` has to be the whole control word: what follows it is the version,
/// normally `1`, or the space or next control word that ends it. `{\rtfoo` is
/// some other word that happens to start the same way.
fn is_rtf(lead: &[u8]) -> bool {
    lead.strip_prefix(RTF)
        .and_then(|rest| rest.first())
        .is_some_and(|next| next.is_ascii_digit() || next.is_ascii_whitespace() || *next == b'\\')
}

/// Text, markup, or nothing this knows.
fn text(head: &[u8], doc: &mut Doc) -> Kind {
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
    let head = match std::str::from_utf8(head) {
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
        Kind::Lit => extension == "lit",
        Kind::Fb2 => extension == "fb2",
        Kind::Rtf => extension == "rtf",
        Kind::Txt => matches!(extension, "txt" | "text" | "md" | "markdown"),
        Kind::Html => matches!(extension, "html" | "htm" | "xhtml" | "xml"),
        Kind::Ole2 => matches!(extension, "doc" | "xls" | "ppt"),
        Kind::ZipWrappedPdf => extension == "zip",
        Kind::ZipWrappedFb2 => matches!(extension, "zip" | "fbz"),
        Kind::Cbz => extension == "cbz",
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
        Kind::Lit => "Microsoft Reader",
        Kind::Fb2 => "FictionBook",
        Kind::Rtf => "RTF",
        Kind::Txt => "plain text",
        Kind::Html => "HTML",
        Kind::Ole2 => "an OLE2 document",
        Kind::ZipWrappedPdf => "a zip holding one PDF",
        Kind::ZipWrappedFb2 => "a zip holding one FictionBook",
        Kind::Cbz => "a comic book archive",
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

    fn archive_of(names: &[&str], name: &str) -> Doc {
        sniff(&zip::tests::zip(names).as_slice(), name)
    }

    #[test]
    fn rtf_is_recognised_by_its_opening_group() {
        assert_eq!(kind_of(br"{\rtf1\ansi\deff0 {\fonttbl", "a.rtf"), Kind::Rtf);
        assert_eq!(
            kind_of(b"\xef\xbb\xbf\r\n  {\\rtf1\\ansi", "a.rtf"),
            Kind::Rtf
        );
        // Eight-bit text in the body does not stop it being RTF.
        assert_eq!(kind_of(b"{\\rtf1 caf\xe9 \x00", "a.rtf"), Kind::Rtf);
    }

    #[test]
    fn rtf_cut_short_of_its_control_word_is_not_rtf() {
        assert_eq!(sniff(&br"{\rt".as_slice(), "a.rtf").kind, Kind::Txt);
        assert_eq!(sniff(&br"{\rtf".as_slice(), "a.rtf").kind, Kind::Txt);
        assert_eq!(kind_of(br"{\rtx1", "a.rtf"), Kind::Txt);
        assert_eq!(kind_of(br"text {\rtf1", "a.rtf"), Kind::Txt);
    }

    #[test]
    fn rtf_is_the_whole_control_word() {
        assert_eq!(kind_of(br"{\rtf\ansi\deff0", "a.rtf"), Kind::Rtf);
        assert_eq!(kind_of(b"{\\rtf \\ansi", "a.rtf"), Kind::Rtf);
        assert_eq!(kind_of(b"{\\rtf\r\n\\ansi", "a.rtf"), Kind::Rtf);
        assert_eq!(kind_of(br"{\rtf2", "a.rtf"), Kind::Rtf);

        assert_eq!(kind_of(br"{\rtfoo bar}", "a.rtf"), Kind::Txt);
        assert_eq!(kind_of(br"{\rtf}", "a.rtf"), Kind::Txt);
        assert_eq!(kind_of(br"{\rtf-1", "a.rtf"), Kind::Txt);
    }

    #[test]
    fn rtf_named_as_a_word_document_is_remarked_on() {
        let doc = sniff(&padded(br"{\rtf1\ansi").as_slice(), "letter.doc");
        assert_eq!(doc.kind, Kind::Rtf);
        assert!(
            doc.notes
                .iter()
                .any(|note| note == "named .doc, but the bytes say RTF"),
            "{:?}",
            doc.notes
        );
    }

    #[test]
    fn microsoft_reader_is_recognised_by_its_signature() {
        let doc = sniff(&padded(b"ITOLITLS\x01\x00\x00\x00").as_slice(), "book.lit");
        assert_eq!(doc.kind, Kind::Lit);
        assert!(doc.notes.is_empty(), "{:?}", doc.notes);
        assert_eq!(kind_of(b"ITOLITL", "book.lit"), Kind::Txt);
        assert_eq!(kind_of(b" ITOLITLS", "book.lit"), Kind::Txt);
    }

    #[test]
    fn a_fictionbook_is_recognised_and_other_xml_is_not() {
        let book = b"<?xml version=\"1.0\" encoding=\"windows-1251\"?>\n\
            <FictionBook xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\">\
            <book-title>\xcc\xe5\xeb\xfc\xed\xe8\xf6\xe0</book-title>";
        let doc = sniff(&padded(book).as_slice(), "mill.fb2");
        assert_eq!(doc.kind, Kind::Fb2);
        assert!(doc.notes.is_empty(), "{:?}", doc.notes);

        let feed = b"<?xml version=\"1.0\"?>\n<rss version=\"2.0\"><channel>";
        assert_eq!(kind_of(feed, "feed.xml"), Kind::Html);
    }

    #[test]
    fn a_utf16_fictionbook_is_recognised_in_either_byte_order() {
        let book = "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n<FictionBook>";
        for big_endian in [false, true] {
            let mut bytes = if big_endian {
                vec![0xfe, 0xff]
            } else {
                vec![0xff, 0xfe]
            };
            for unit in book.encode_utf16() {
                bytes.extend_from_slice(&if big_endian {
                    unit.to_be_bytes()
                } else {
                    unit.to_le_bytes()
                });
            }
            let doc = sniff(&padded(&bytes).as_slice(), "mill.fb2");
            assert_eq!(doc.kind, Kind::Fb2, "big-endian: {big_endian}");
            assert!(doc.notes.is_empty(), "{:?}", doc.notes);
        }
    }

    #[test]
    fn a_fictionbook_named_as_something_else_is_remarked_on() {
        let doc = sniff(&padded(b"<FictionBook>").as_slice(), "mill.txt");
        assert_eq!(doc.kind, Kind::Fb2);
        assert!(
            doc.notes.iter().any(|note| note.contains("FictionBook")),
            "{:?}",
            doc.notes
        );
    }

    #[test]
    fn a_zip_holding_one_fictionbook_is_a_wrapped_one() {
        let doc = archive_of(&["__MACOSX/._mill.fb2", "mill.fb2"], "mill.fb2.zip");
        assert_eq!(doc.kind, Kind::ZipWrappedFb2);
        assert_eq!(doc.declared_ext.as_deref(), Some("zip"));
        assert!(
            doc.notes
                .iter()
                .any(|note| note.contains("one FictionBook")),
            "{:?}",
            doc.notes
        );
        assert!(!doc.notes.iter().any(|note| note.contains("named .")));

        let two = archive_of(&["one.fb2", "two.fb2"], "pair.zip");
        assert_eq!(two.kind, Kind::Unknown);
    }

    #[test]
    fn a_zip_of_page_images_is_a_comic() {
        let doc = archive_of(
            &[
                "Issue 1/",
                "Issue 1/001.jpg",
                "Issue 1/002.JPEG",
                "Issue 1/003.png",
                "Issue 1/004.gif",
                "Issue 1/005.webp",
                "ComicInfo.xml",
                "__MACOSX/Issue 1/._001.jpg",
                "Issue 1/Thumbs.db",
                ".DS_Store",
            ],
            "issue-1.cbz",
        );
        assert_eq!(doc.kind, Kind::Cbz);
        assert!(
            !doc.notes.iter().any(|note| note.contains("named .")),
            "{:?}",
            doc.notes
        );
    }

    #[test]
    fn a_zip_of_photographs_is_not_a_comic() {
        let doc = archive_of(&["IMG_0001.jpg", "IMG_0002.jpg"], "holiday.zip");
        assert_eq!(doc.kind, Kind::Unknown);
        assert!(
            doc.notes.iter().any(|note| note
                == "a zip of 2 images, which would be a comic book if its name said .cbz"),
            "{:?}",
            doc.notes
        );

        let one = archive_of(&["IMG_0001.jpg"], "holiday.zip");
        assert!(
            one.notes
                .iter()
                .any(|note| note.starts_with("a zip of 1 image,")),
            "{:?}",
            one.notes
        );
    }

    #[test]
    fn the_same_images_named_as_a_comic_are_one() {
        let doc = archive_of(&["IMG_0001.jpg", "IMG_0002.jpg"], "holiday.cbz");
        assert_eq!(doc.kind, Kind::Cbz);
        assert!(
            !doc.notes.iter().any(|note| note.contains("named .")),
            "{:?}",
            doc.notes
        );

        // A comic zipped and named as if it were RAR is still a comic, and
        // the name is remarked on.
        let rar = archive_of(&["001.jpg", "002.jpg"], "issue-1.cbr");
        assert_eq!(rar.kind, Kind::Cbz);
        assert!(
            rar.notes.iter().any(|note| note.contains("named .cbr")),
            "{:?}",
            rar.notes
        );
    }

    #[test]
    fn a_comic_saved_as_a_zip_is_known_by_its_catalogue_and_remarked_on() {
        let doc = archive_of(&["001.jpg", "002.jpg", "ComicInfo.xml"], "issue-1.zip");
        assert_eq!(doc.kind, Kind::Cbz);
        assert!(
            doc.notes
                .iter()
                .any(|note| note == "named .zip, but the bytes say a comic book archive"),
            "{:?}",
            doc.notes
        );
    }

    #[test]
    fn a_zip_with_anything_but_images_is_not_a_comic() {
        let one_pdf = archive_of(&["001.jpg", "002.jpg", "extras.pdf"], "issue-1.cbz");
        assert_eq!(one_pdf.kind, Kind::Unknown);
        assert!(
            one_pdf
                .notes
                .iter()
                .any(|note| note.contains("none of them a book")),
            "{:?}",
            one_pdf.notes
        );

        let readme = archive_of(&["001.jpg", "readme.txt"], "issue-1.cbz");
        assert_eq!(readme.kind, Kind::Unknown);

        // A name that ends like an image without being one.
        let lookalike = archive_of(&["001.jpg", "notes.jpg.txt"], "issue-1.cbz");
        assert_eq!(lookalike.kind, Kind::Unknown);
    }

    #[test]
    fn a_zip_with_no_pages_is_not_a_comic() {
        let catalogue_only = archive_of(&["ComicInfo.xml", "pages/"], "issue-1.cbz");
        assert_eq!(catalogue_only.kind, Kind::Unknown);

        let empty = archive_of(&[], "issue-1.cbz");
        assert_eq!(empty.kind, Kind::Unknown);
    }

    #[test]
    fn a_truncated_comic_does_not_panic() {
        let whole = zip::tests::zip(&["001.jpg", "002.png", "ComicInfo.xml"]);
        for cut in 0..whole.len() {
            let _ = sniff(&&whole[..cut], "issue-1.cbz");
        }
    }
}
