//! A Kindle book's MOBI header and the EXTH records after it.
//!
//! MOBI, AZW and AZW3 files are Palm databases. The first record opens with a
//! PalmDOC header and a MOBI header, which gives the book's full name and says
//! how its strings are encoded. When a flag in that header is set, an EXTH
//! block follows with everything else: authors, publisher, ISBN, subjects, and
//! which image record is the cover. AZW3 (KF8) keeps the same layout in its
//! first record, so one reader covers all three.
//!
//! Every offset in the file is checked before it is followed. A file whose
//! headers are cut short or written wrongly gives an empty record with a note;
//! an EXTH block that breaks off keeps the records before the break. Neither
//! panics or reads past the end.

use crate::epub::{digits_of, is_isbn};
use crate::{Cover, Metadata, Source};

/// Where a Palm database says what it holds.
const TYPE_AT: usize = 60;
/// What a Kindle book says there.
const BOOK_TYPE: &[u8] = b"BOOKMOBI";
/// The Palm database's own name: 32 bytes, NUL-padded.
const NAME_LEN: usize = 32;
/// Where the number of records is written.
const RECORD_COUNT_AT: usize = 76;
/// Where the record list starts: eight bytes per record, offset first.
const RECORD_LIST_AT: usize = 78;

/// Where the PalmDOC header says whether the text is encrypted.
const ENCRYPTION_AT: usize = 12;
/// Where the MOBI header begins inside the first record.
const MOBI_AT: usize = 16;
/// The MOBI header's own magic.
const MOBI_MAGIC: &[u8] = b"MOBI";

// Fields of the MOBI header, as offsets from its start. Each is only there
// when the header's declared length reaches past it.
const HEADER_LENGTH: usize = 4;
const TEXT_ENCODING: usize = 12;
const FULL_NAME_OFFSET: usize = 68;
const FULL_NAME_LENGTH: usize = 72;
const LOCALE: usize = 76;
const FIRST_IMAGE: usize = 92;
const EXTH_FLAGS: usize = 112;

/// The flag that says an EXTH block follows the MOBI header.
const HAS_EXTH: u32 = 0x40;
/// The EXTH block's magic.
const EXTH_MAGIC: &[u8] = b"EXTH";

/// The text encodings a MOBI header names.
const UTF_8: u32 = 65001;
const WINDOWS_1252: u32 = 1252;

/// What an index or offset says when it points at nothing.
const NONE: u32 = 0xffff_ffff;

// EXTH record types.
const AUTHOR: u32 = 100;
const PUBLISHER: u32 = 101;
const DESCRIPTION: u32 = 103;
const ISBN: u32 = 104;
const SUBJECT: u32 = 105;
const PUBLISHED: u32 = 106;
const ASIN: u32 = 113;
const COVER_OFFSET: u32 = 201;
const THUMB_OFFSET: u32 = 202;
const UPDATED_TITLE: u32 = 503;
const LANGUAGE: u32 = 524;

/// Whether these bytes are a Kindle book this module can read.
pub fn is_mobi(bytes: &[u8]) -> bool {
    bytes.get(TYPE_AT..TYPE_AT + BOOK_TYPE.len()) == Some(BOOK_TYPE)
}

/// Read what a MOBI, AZW or AZW3 file says about itself.
pub fn read(path: &str, bytes: &[u8]) -> Metadata {
    let mut meta = Metadata::empty(path, Source::Mobi);
    if let Err(why) = fill(&mut meta, bytes) {
        let mut empty = Metadata::empty(path, Source::None);
        empty.note(why);
        return empty;
    }
    if meta.is_empty() {
        meta.note("a MOBI header with nothing filled in");
    }
    meta
}

/// The strings an EXTH block carries, before they are placed in a record.
#[derive(Default)]
struct Exth {
    authors: Vec<String>,
    publisher: Option<String>,
    description: Option<String>,
    isbn: Option<String>,
    subjects: Vec<String>,
    published: Option<String>,
    asin: Option<String>,
    cover: Option<u32>,
    thumb: Option<u32>,
    title: Option<String>,
    language: Option<String>,
}

fn fill(meta: &mut Metadata, bytes: &[u8]) -> Result<(), String> {
    if !is_mobi(bytes) {
        return Err("not a Kindle book: the Palm database type is not BOOKMOBI".into());
    }
    let records = records(bytes)?;
    let first = record(bytes, &records, 0).ok_or("the first record lies outside the file")?;

    if first.get(MOBI_AT..MOBI_AT + MOBI_MAGIC.len()) != Some(MOBI_MAGIC) {
        return Err("no MOBI header in the first record".into());
    }
    let header_length =
        u32_at(first, MOBI_AT + HEADER_LENGTH).ok_or("the MOBI header is cut short")? as usize;
    let header = Header {
        record: first,
        length: header_length,
    };

    let encoding = header.field(TEXT_ENCODING).unwrap_or(WINDOWS_1252);
    if encoding != UTF_8 && encoding != WINDOWS_1252 {
        meta.note(format!(
            "text encoding {encoding}, which is neither UTF-8 nor Windows-1252; read as Windows-1252"
        ));
    }
    let decode = |raw: &[u8]| text(raw, encoding);

    if u16_at(first, ENCRYPTION_AT).is_some_and(|kind| kind != 0) {
        meta.note("drm");
    }

    let exth = if header
        .field(EXTH_FLAGS)
        .is_some_and(|flags| flags & HAS_EXTH != 0)
    {
        let at = MOBI_AT
            .checked_add(header_length)
            .ok_or("the MOBI header length is out of range")?;
        exth(first, at, &decode, meta)
    } else {
        Exth::default()
    };

    let full_name = header
        .field(FULL_NAME_OFFSET)
        .zip(header.field(FULL_NAME_LENGTH))
        .and_then(|(at, len)| slice(first, at as usize, len as usize))
        .and_then(decode);
    meta.title = match (exth.title, full_name) {
        (Some(title), _) | (None, Some(title)) => Some(title),
        (None, None) => {
            let name = palm_name(bytes);
            if name.is_some() {
                meta.note("title from the Palm database name, which is cut to 31 bytes");
            }
            name
        }
    };

    meta.authors = exth.authors;
    meta.publisher = exth.publisher;
    meta.description = exth.description;
    meta.date = exth.published;
    meta.subjects = exth.subjects;
    meta.language = exth.language.or_else(|| {
        header
            .field(LOCALE)
            .and_then(|locale| language_of(locale).map(str::to_string))
    });
    if let Some(isbn) = exth.isbn {
        if is_isbn(&isbn) {
            meta.identifiers.insert("isbn".into(), digits_of(&isbn));
        } else {
            meta.identifiers.insert("other".into(), isbn);
        }
    }
    if let Some(asin) = exth.asin {
        meta.identifiers.insert("asin".into(), asin);
    }

    if let Some(offset) = exth.cover.or(exth.thumb) {
        meta.cover = cover(bytes, &records, header.field(FIRST_IMAGE), offset, meta);
    }
    Ok(())
}

/// The MOBI header, for reading fields that may or may not be in it.
struct Header<'a> {
    record: &'a [u8],
    length: usize,
}

impl Header<'_> {
    /// A field at `at` bytes into the header, when the header reaches it.
    fn field(&self, at: usize) -> Option<u32> {
        if at.checked_add(4)? > self.length {
            return None;
        }
        u32_at(self.record, MOBI_AT.checked_add(at)?)
    }
}

/// Where each record starts, as the record list says.
fn records(bytes: &[u8]) -> Result<Vec<usize>, String> {
    let count = u16_at(bytes, RECORD_COUNT_AT).ok_or("the Palm database header is cut short")?;
    if count == 0 {
        return Err("a Palm database with no records".into());
    }
    let mut out = Vec::with_capacity(count as usize);
    for at in 0..count as usize {
        let entry = RECORD_LIST_AT + at * 8;
        let Some(offset) = u32_at(bytes, entry) else {
            return Err("the record list runs past the end of the file".into());
        };
        out.push(offset as usize);
    }
    Ok(out)
}

/// One record's bytes: from its offset to the next record's, or to the end.
fn record<'a>(bytes: &'a [u8], records: &[usize], index: usize) -> Option<&'a [u8]> {
    let start = *records.get(index)?;
    let end = records.get(index + 1).copied().unwrap_or(bytes.len());
    bytes.get(start..end)
}

/// Read the EXTH block at `at` inside the first record.
///
/// A block that ends early keeps what was read before the break, with a note:
/// the records already read are as good as they would have been.
fn exth(
    first: &[u8],
    at: usize,
    decode: &dyn Fn(&[u8]) -> Option<String>,
    meta: &mut Metadata,
) -> Exth {
    let mut out = Exth::default();
    if at >= first.len() || first.get(at..at + EXTH_MAGIC.len()) != Some(EXTH_MAGIC) {
        meta.note("the header says an EXTH block follows, but none does");
        return out;
    }
    let (Some(length), Some(count)) = (u32_at(first, at + 4), u32_at(first, at + 8)) else {
        meta.note("the EXTH block ends early");
        return out;
    };
    let end = at.saturating_add(length as usize).min(first.len());

    let mut cursor = at + 12;
    for _ in 0..count {
        let (Some(kind), Some(size)) = (u32_at(first, cursor), u32_at(first, cursor + 4)) else {
            meta.note("the EXTH block ends early");
            break;
        };
        let size = size as usize;
        let Some(data) = size
            .checked_sub(8)
            .and_then(|len| slice(first, cursor + 8, len))
            .filter(|_| cursor.checked_add(size).is_some_and(|stop| stop <= end))
        else {
            meta.note("the EXTH block ends early");
            break;
        };
        cursor += size;

        match kind {
            AUTHOR => out.authors.extend(decode(data)),
            SUBJECT => out.subjects.extend(decode(data)),
            PUBLISHER => set(&mut out.publisher, decode(data)),
            DESCRIPTION => set(&mut out.description, decode(data)),
            ISBN => set(&mut out.isbn, decode(data)),
            PUBLISHED => set(&mut out.published, decode(data)),
            ASIN => set(&mut out.asin, decode(data)),
            UPDATED_TITLE => set(&mut out.title, decode(data)),
            LANGUAGE => set(&mut out.language, decode(data)),
            COVER_OFFSET => set(&mut out.cover, number(data)),
            THUMB_OFFSET => set(&mut out.thumb, number(data)),
            _ => {}
        }
    }
    out
}

/// Keep the first value a field is given: a later duplicate is usually a
/// converter writing the same thing twice, and the first is what it meant.
fn set<T>(slot: &mut Option<T>, value: Option<T>) {
    if slot.is_none() {
        *slot = value;
    }
}

/// A four-byte number record that points somewhere.
fn number(data: &[u8]) -> Option<u32> {
    let value = u32::from_be_bytes(data.try_into().ok()?);
    (value != NONE).then_some(value)
}

/// The cover image, from the record the header and the EXTH offset name.
fn cover(
    bytes: &[u8],
    records: &[usize],
    first_image: Option<u32>,
    offset: u32,
    meta: &mut Metadata,
) -> Option<Cover> {
    let Some(first_image) = first_image.filter(|&index| index != NONE) else {
        meta.note("a cover is named, but the header has no image records");
        return None;
    };
    let Some(data) = first_image
        .checked_add(offset)
        .and_then(|index| record(bytes, records, index as usize))
    else {
        meta.note("the cover points past the last record");
        return None;
    };
    let Some(media_type) = image_type(data) else {
        meta.note("the cover record is not an image");
        return None;
    };
    Some(Cover {
        media_type: media_type.to_string(),
        bytes: data.to_vec(),
    })
}

/// What kind of image these bytes are, by their magic.
fn image_type(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if data.starts_with(b"BM") && data.len() > 14 {
        Some("image/bmp")
    } else {
        None
    }
}

/// The Palm database's name, the last place a title can come from.
///
/// Converters write the title there with spaces made underscores; it is
/// reported as written.
fn palm_name(bytes: &[u8]) -> Option<String> {
    let raw = bytes.get(..NAME_LEN)?;
    let raw = raw.split(|&byte| byte == 0).next().unwrap_or(raw);
    text(raw, WINDOWS_1252)
}

/// A string in the book's declared encoding, trimmed; empty is none.
fn text(raw: &[u8], encoding: u32) -> Option<String> {
    let decoded: String = match (encoding, std::str::from_utf8(raw)) {
        (UTF_8, Ok(text)) => text.to_string(),
        (UTF_8, Err(_)) => String::from_utf8_lossy(raw).into_owned(),
        _ => raw.iter().map(|&byte| windows_1252(byte)).collect(),
    };
    let trimmed = decoded.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// One Windows-1252 byte as a character.
fn windows_1252(byte: u8) -> char {
    match byte {
        0x80..=0x9f => HIGH_CONTROLS[(byte - 0x80) as usize],
        _ => byte as char,
    }
}

/// The characters Windows-1252 puts where Latin-1 has C1 controls. The five
/// positions it leaves undefined decode to themselves, as browsers do.
const HIGH_CONTROLS: [char; 32] = [
    '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}',
    '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
];

/// The language a MOBI locale names, for books with no EXTH language.
///
/// The low byte is a Windows primary language id; the region above it is
/// dropped, since the file's own language field never carries one either.
fn language_of(locale: u32) -> Option<&'static str> {
    Some(match locale & 0xff {
        0x01 => "ar",
        0x02 => "bg",
        0x03 => "ca",
        0x04 => "zh",
        0x05 => "cs",
        0x06 => "da",
        0x07 => "de",
        0x08 => "el",
        0x09 => "en",
        0x0a => "es",
        0x0b => "fi",
        0x0c => "fr",
        0x0d => "he",
        0x0e => "hu",
        0x0f => "is",
        0x10 => "it",
        0x11 => "ja",
        0x12 => "ko",
        0x13 => "nl",
        0x14 => "no",
        0x15 => "pl",
        0x16 => "pt",
        0x18 => "ro",
        0x19 => "ru",
        0x1a => "hr",
        0x1b => "sk",
        0x1d => "sv",
        0x1f => "tr",
        0x22 => "uk",
        0x29 => "fa",
        0x2a => "vi",
        0x36 => "af",
        0x39 => "hi",
        _ => return None,
    })
}

fn slice(bytes: &[u8], at: usize, len: usize) -> Option<&[u8]> {
    bytes.get(at..at.checked_add(len)?)
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(slice(bytes, at, 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(slice(bytes, at, 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F'];

    /// A Kindle book built field by field, so each test can break one thing.
    struct Book {
        name: &'static [u8],
        encoding: u32,
        encryption: u16,
        locale: u32,
        full_name: Vec<u8>,
        exth: Option<Vec<(u32, Vec<u8>)>>,
        first_image: u32,
        images: Vec<Vec<u8>>,
    }

    impl Default for Book {
        fn default() -> Self {
            Book {
                name: b"A_Test_Book",
                encoding: UTF_8,
                encryption: 0,
                locale: 0x09,
                full_name: b"A Test Book".to_vec(),
                exth: Some(Vec::new()),
                first_image: 1,
                images: Vec::new(),
            }
        }
    }

    impl Book {
        fn bytes(&self) -> Vec<u8> {
            const HEADER: usize = 232;
            let mut first = vec![0u8; MOBI_AT + HEADER];
            first[ENCRYPTION_AT..ENCRYPTION_AT + 2].copy_from_slice(&self.encryption.to_be_bytes());
            first[MOBI_AT..MOBI_AT + 4].copy_from_slice(MOBI_MAGIC);
            let put = |first: &mut Vec<u8>, at: usize, value: u32| {
                first[MOBI_AT + at..MOBI_AT + at + 4].copy_from_slice(&value.to_be_bytes());
            };
            put(&mut first, HEADER_LENGTH, HEADER as u32);
            put(&mut first, TEXT_ENCODING, self.encoding);
            put(&mut first, LOCALE, self.locale);
            put(&mut first, FIRST_IMAGE, self.first_image);
            if let Some(records) = &self.exth {
                put(&mut first, EXTH_FLAGS, HAS_EXTH);
                let mut body = Vec::new();
                for (kind, data) in records {
                    body.extend_from_slice(&kind.to_be_bytes());
                    body.extend_from_slice(&(data.len() as u32 + 8).to_be_bytes());
                    body.extend_from_slice(data);
                }
                first.extend_from_slice(EXTH_MAGIC);
                first.extend_from_slice(&(body.len() as u32 + 12).to_be_bytes());
                first.extend_from_slice(&(records.len() as u32).to_be_bytes());
                first.extend_from_slice(&body);
            }
            let name_at = first.len();
            first.extend_from_slice(&self.full_name);
            put(&mut first, FULL_NAME_OFFSET, name_at as u32);
            put(&mut first, FULL_NAME_LENGTH, self.full_name.len() as u32);

            let mut records = vec![first];
            records.extend(self.images.iter().cloned());

            let mut out = vec![0u8; RECORD_LIST_AT + records.len() * 8];
            out[..self.name.len()].copy_from_slice(self.name);
            out[TYPE_AT..TYPE_AT + 8].copy_from_slice(BOOK_TYPE);
            out[RECORD_COUNT_AT..RECORD_COUNT_AT + 2]
                .copy_from_slice(&(records.len() as u16).to_be_bytes());
            for (at, record) in records.iter().enumerate() {
                let offset = out.len() as u32;
                let entry = RECORD_LIST_AT + at * 8;
                out[entry..entry + 4].copy_from_slice(&offset.to_be_bytes());
                out.extend_from_slice(record);
            }
            out
        }
    }

    fn exth(records: &[(u32, &[u8])]) -> Option<Vec<(u32, Vec<u8>)>> {
        Some(records.iter().map(|(k, v)| (*k, v.to_vec())).collect())
    }

    #[test]
    fn the_exth_records_fill_the_record() {
        let book = Book {
            exth: exth(&[
                (UPDATED_TITLE, b"The Real Title"),
                (AUTHOR, b"First Author"),
                (AUTHOR, b"Second Author"),
                (PUBLISHER, b"A Press"),
                (DESCRIPTION, b"What it is about."),
                (ISBN, b"978-0-306-40615-7"),
                (SUBJECT, b"History"),
                (SUBJECT, b"Ships"),
                (PUBLISHED, b"1999-01-02"),
                (ASIN, b"B000TEST00"),
                (LANGUAGE, b"de"),
            ]),
            ..Book::default()
        };
        let meta = read("x.mobi", &book.bytes());
        assert_eq!(meta.source, "mobi");
        assert_eq!(meta.title.as_deref(), Some("The Real Title"));
        assert_eq!(meta.authors, ["First Author", "Second Author"]);
        assert_eq!(meta.publisher.as_deref(), Some("A Press"));
        assert_eq!(meta.description.as_deref(), Some("What it is about."));
        assert_eq!(meta.date.as_deref(), Some("1999-01-02"));
        assert_eq!(meta.subjects, ["History", "Ships"]);
        assert_eq!(meta.language.as_deref(), Some("de"));
        assert_eq!(meta.identifiers["isbn"], "9780306406157");
        assert_eq!(meta.identifiers["asin"], "B000TEST00");
        assert!(meta.notes.is_empty(), "{:?}", meta.notes);
    }

    #[test]
    fn the_title_falls_back_to_the_full_name_and_then_the_database_name() {
        let meta = read("x.mobi", &Book::default().bytes());
        assert_eq!(meta.title.as_deref(), Some("A Test Book"));

        let book = Book {
            full_name: Vec::new(),
            ..Book::default()
        };
        let meta = read("x.mobi", &book.bytes());
        assert_eq!(meta.title.as_deref(), Some("A_Test_Book"));
        assert!(meta.notes[0].contains("Palm database name"));
    }

    #[test]
    fn the_language_falls_back_to_the_locale() {
        let book = Book {
            locale: 0x040c,
            ..Book::default()
        };
        assert_eq!(
            read("x.mobi", &book.bytes()).language.as_deref(),
            Some("fr")
        );

        let book = Book {
            locale: 0,
            ..Book::default()
        };
        assert_eq!(read("x.mobi", &book.bytes()).language, None);
    }

    #[test]
    fn windows_1252_strings_decode_as_windows_1252() {
        let book = Book {
            encoding: WINDOWS_1252,
            exth: exth(&[
                (AUTHOR, b"Ren\xe9 Ch\x96ar"),
                (PUBLISHER, b"\x93Quoted\x94"),
            ]),
            ..Book::default()
        };
        let meta = read("x.mobi", &book.bytes());
        assert_eq!(meta.authors, ["René Ch–ar"]);
        assert_eq!(meta.publisher.as_deref(), Some("“Quoted”"));
    }

    #[test]
    fn the_cover_is_the_image_record_the_offset_names() {
        let book = Book {
            exth: exth(&[(COVER_OFFSET, &1u32.to_be_bytes())]),
            images: vec![b"GIF89a-thumb".to_vec(), JPEG.to_vec()],
            ..Book::default()
        };
        let cover = read("x.mobi", &book.bytes()).cover.expect("a cover");
        assert_eq!(cover.media_type, "image/jpeg");
        assert_eq!(cover.bytes, JPEG);
    }

    #[test]
    fn the_thumbnail_stands_in_when_no_cover_is_named() {
        let book = Book {
            exth: exth(&[(THUMB_OFFSET, &0u32.to_be_bytes())]),
            images: vec![b"\x89PNG\r\n\x1a\nrest".to_vec()],
            ..Book::default()
        };
        let cover = read("x.mobi", &book.bytes()).cover.expect("a cover");
        assert_eq!(cover.media_type, "image/png");
    }

    #[test]
    fn a_cover_that_points_nowhere_is_a_note_and_not_a_cover() {
        let book = Book {
            exth: exth(&[(COVER_OFFSET, &7u32.to_be_bytes())]),
            images: vec![JPEG.to_vec()],
            ..Book::default()
        };
        let meta = read("x.mobi", &book.bytes());
        assert_eq!(meta.cover, None);
        assert_eq!(meta.notes, ["the cover points past the last record"]);

        let book = Book {
            exth: exth(&[(COVER_OFFSET, &u32::MAX.to_be_bytes())]),
            first_image: u32::MAX - 1,
            images: vec![JPEG.to_vec()],
            ..Book::default()
        };
        assert_eq!(read("x.mobi", &book.bytes()).cover, None);

        let book = Book {
            exth: exth(&[(COVER_OFFSET, &0u32.to_be_bytes())]),
            images: vec![b"not an image".to_vec()],
            ..Book::default()
        };
        let meta = read("x.mobi", &book.bytes());
        assert_eq!(meta.cover, None);
        assert_eq!(meta.notes, ["the cover record is not an image"]);
    }

    #[test]
    fn an_encrypted_book_still_has_its_header_read() {
        let book = Book {
            encryption: 2,
            exth: exth(&[(AUTHOR, b"Somebody")]),
            ..Book::default()
        };
        let meta = read("x.azw", &book.bytes());
        assert_eq!(meta.authors, ["Somebody"]);
        assert_eq!(meta.notes, ["drm"]);
    }

    #[test]
    fn an_exth_record_that_overruns_its_block_stops_the_reading() {
        let mut bytes = Book {
            exth: exth(&[(AUTHOR, b"Kept"), (PUBLISHER, b"Lost")]),
            ..Book::default()
        }
        .bytes();
        // The second record's length, made to reach past the block.
        let at = find(&bytes, b"Lost") - 4;
        bytes[at..at + 4].copy_from_slice(&0x7fff_ffffu32.to_be_bytes());
        let meta = read("x.mobi", &bytes);
        assert_eq!(meta.authors, ["Kept"]);
        assert_eq!(meta.publisher, None);
        assert!(
            meta.notes
                .contains(&"the EXTH block ends early".to_string())
        );
    }

    #[test]
    fn a_record_length_shorter_than_its_own_header_stops_the_reading() {
        let mut bytes = Book {
            exth: exth(&[(AUTHOR, b"Kept")]),
            ..Book::default()
        }
        .bytes();
        let at = find(&bytes, b"Kept") - 4;
        bytes[at..at + 4].copy_from_slice(&3u32.to_be_bytes());
        let meta = read("x.mobi", &bytes);
        assert!(meta.authors.is_empty());
        assert!(
            meta.notes
                .contains(&"the EXTH block ends early".to_string())
        );
    }

    #[test]
    fn what_is_not_a_kindle_book_says_so() {
        let meta = read("x.mobi", b"not a palm database at all");
        assert_eq!(meta.source, "none");
        assert!(meta.notes[0].starts_with("not a Kindle book"));
    }

    #[test]
    fn a_first_record_with_no_mobi_header_is_refused() {
        let mut bytes = Book::default().bytes();
        let at = mobi_header(&bytes);
        bytes[at] = b'X';
        let meta = read("x.mobi", &bytes);
        assert_eq!(meta.source, "none");
        assert_eq!(meta.notes, ["no MOBI header in the first record"]);
    }

    #[test]
    fn a_header_too_short_for_a_field_does_not_have_that_field() {
        let mut bytes = Book::default().bytes();
        let at = mobi_header(&bytes) + HEADER_LENGTH;
        // Long enough for the encoding, too short for the full name.
        bytes[at..at + 4].copy_from_slice(&24u32.to_be_bytes());
        let meta = read("x.mobi", &bytes);
        assert_eq!(meta.title.as_deref(), Some("A_Test_Book"));
        assert_eq!(meta.language, None);
    }

    #[test]
    fn every_truncation_returns_without_panicking() {
        let bytes = Book {
            exth: exth(&[
                (UPDATED_TITLE, b"Title"),
                (AUTHOR, b"Author"),
                (COVER_OFFSET, &0u32.to_be_bytes()),
            ]),
            images: vec![JPEG.to_vec()],
            ..Book::default()
        }
        .bytes();
        for len in 0..=bytes.len() {
            let _ = read("x.mobi", &bytes[..len]);
        }
    }

    /// Where the MOBI header starts: after the record list, so the `MOBI`
    /// at the end of the database type is not it.
    fn mobi_header(bytes: &[u8]) -> usize {
        RECORD_LIST_AT + find(&bytes[RECORD_LIST_AT..], MOBI_MAGIC)
    }

    fn find(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("present")
    }
}
