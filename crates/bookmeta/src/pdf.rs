//! A PDF's information dictionary.
//!
//! Older PDFs put the title and author in a dictionary named by the trailer.
//! Newer ones prefer XMP, an RDF document embedded as a stream, and most
//! carry both. The dictionary is read here because it is where a converter
//! writes, and what it writes is usually the only thing in the file.
//!
//! What is written there is often rubbish — `Microsoft Word - ch3.doc`, a
//! DTP scratch path, the name of whoever's licence produced it. That is
//! reported as it stands, with a note saying where it came from.

use lopdf::{Document, Object};

use crate::{Metadata, Source};

/// Read what a PDF says about itself.
pub fn read(path: &str, bytes: &[u8]) -> Metadata {
    let mut meta = Metadata::empty(path, Source::PdfInfo);

    let document = match Document::load_mem(bytes) {
        Ok(document) => document,
        Err(e) => {
            meta.source = Source::None.name().to_string();
            meta.note(format!("could not be parsed: {e}"));
            return meta;
        }
    };
    if document.is_encrypted() {
        meta.source = Source::None.name().to_string();
        meta.note("encrypted, so nothing can be read out of it");
        return meta;
    }

    let Some(info) = info_dictionary(&document) else {
        meta.source = Source::None.name().to_string();
        meta.note("no information dictionary");
        return meta;
    };

    meta.title = text(&document, &info, b"Title");
    meta.authors = text(&document, &info, b"Author")
        .map(|author| split_authors(&author))
        .unwrap_or_default();
    meta.date = text(&document, &info, b"CreationDate").map(|date| tidy_date(&date));
    meta.subjects = text(&document, &info, b"Keywords")
        .map(|keywords| split_keywords(&keywords))
        .unwrap_or_default();

    if let Some(producer) = text(&document, &info, b"Producer") {
        meta.note(format!("produced by {producer}"));
    }
    if meta.is_empty() {
        meta.note("an information dictionary with nothing filled in");
    }
    meta
}

/// The trailer's `/Info` dictionary, dereferenced.
fn info_dictionary(document: &Document) -> Option<lopdf::Dictionary> {
    let info = document.trailer.get(b"Info").ok()?;
    let (_, object) = document.dereference(info).ok()?;
    object.as_dict().ok().cloned()
}

/// One entry, as text, with the two PDF string encodings both handled.
fn text(document: &Document, info: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    let object = info.get(key).ok()?;
    let (_, object) = document.dereference(object).ok()?;
    let bytes = match object {
        Object::String(bytes, _) => bytes,
        _ => return None,
    };
    let decoded = decode(bytes);
    let trimmed = decoded.trim();
    match trimmed.is_empty() {
        true => None,
        false => Some(trimmed.to_string()),
    }
}

/// PDFDocEncoding or UTF-16, told apart by the byte-order mark the spec
/// requires on the latter.
fn decode(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    // PDFDocEncoding agrees with Latin-1 over everything a title uses.
    bytes.iter().map(|byte| *byte as char).collect()
}

/// Authors, as a producer felt like writing them.
///
/// There is no convention: semicolons, commas, ampersands and the word "and"
/// all appear. A comma is left alone, because "Last, First" is one person far
/// more often than two.
fn split_authors(author: &str) -> Vec<String> {
    let parts: Vec<String> = author
        .split([';', '&'])
        .flat_map(|part| part.split(" and "))
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect();
    match parts.is_empty() {
        true => Vec::new(),
        false => parts,
    }
}

/// Keywords, which are comma or semicolon separated by convention.
fn split_keywords(keywords: &str) -> Vec<String> {
    keywords
        .split([',', ';'])
        .map(|word| word.trim().to_string())
        .filter(|word| !word.is_empty())
        .collect()
}

/// A PDF date, which is `D:YYYYMMDDHHmmSS` and an offset, reduced to the day.
///
/// Left as written when it is not that shape, because a date nobody can parse
/// is still evidence.
fn tidy_date(date: &str) -> String {
    let digits: String = date
        .trim_start_matches("D:")
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    match digits.len() >= 8 {
        true => format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8]),
        false => date.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pdf_date_is_reduced_to_the_day() {
        assert_eq!(tidy_date("D:19840917120000+01'00'"), "1984-09-17");
        assert_eq!(tidy_date("D:20240102"), "2024-01-02");
    }

    #[test]
    fn a_date_nobody_can_parse_is_kept_as_written() {
        assert_eq!(tidy_date("sometime in 1984"), "sometime in 1984");
        assert_eq!(tidy_date("D:2024"), "D:2024");
    }

    #[test]
    fn authors_are_split_on_what_producers_actually_use() {
        assert_eq!(split_authors("Palmer; Ware"), ["Palmer", "Ware"]);
        assert_eq!(split_authors("Palmer & Ware"), ["Palmer", "Ware"]);
        assert_eq!(split_authors("Palmer and Ware"), ["Palmer", "Ware"]);
    }

    #[test]
    fn a_comma_is_not_a_separator_because_last_first_is_one_person() {
        assert_eq!(split_authors("Dostoevsky, Fyodor"), ["Dostoevsky, Fyodor"]);
    }

    #[test]
    fn utf16_titles_are_decoded_and_latin1_ones_are_too() {
        let utf16 = [0xFE, 0xFF, 0x00, 0x53, 0x00, 0xE9];
        assert_eq!(decode(&utf16), "Sé");
        assert_eq!(decode(b"Plain"), "Plain");
    }

    #[test]
    fn keywords_split_on_either_separator() {
        assert_eq!(
            split_keywords("history, war ; peace"),
            ["history", "war", "peace"]
        );
        assert_eq!(split_keywords(""), Vec::<String>::new());
    }

    #[test]
    fn what_is_not_a_pdf_says_so() {
        let meta = read("x.pdf", b"%PDF-1.4 but truncated");
        assert_eq!(meta.source, "none");
        assert!(!meta.notes.is_empty());
    }
}
