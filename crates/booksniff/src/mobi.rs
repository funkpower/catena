//! Telling the two Kindle formats apart.
//!
//! Both are Palm databases with the same type and creator, so the extension
//! is no help and neither is the first eight bytes. The difference is a
//! version number inside the first record: six is the old format, eight is
//! the one that carries real typesetting.

use corpus_core::Kind;

use crate::bytes::Bytes;

/// Where a Palm database says what it holds.
const TYPE_AT: u64 = 60;
/// What a Kindle book says there.
const BOOK_TYPE: &[u8] = b"BOOKMOBI";

/// Where the first record's offset is written.
const FIRST_RECORD_AT: u64 = 78;

/// The MOBI header's own magic, at the start of the first record's header.
const MOBI_MAGIC: &[u8] = b"MOBI";
/// Where the MOBI header begins inside the first record.
const MOBI_HEADER_AT: u64 = 16;
/// How far into the MOBI header the format version is.
const VERSION_AT: u64 = 36;
/// The version that means the newer format.
const KF8: u32 = 8;

/// Which Kindle format this is, or `None` if it is not one.
pub fn kind(source: &impl Bytes) -> Option<Kind> {
    if !source.magic_at(TYPE_AT, BOOK_TYPE) {
        return None;
    }
    Some(match version(source) {
        Some(version) if version >= KF8 => Kind::Azw3,
        _ => Kind::Mobi,
    })
}

/// The format version in the first record, when it is readable.
fn version(source: &impl Bytes) -> Option<u32> {
    let record = big_endian(source, FIRST_RECORD_AT)? as u64;
    let header = record + MOBI_HEADER_AT;
    if !source.magic_at(header, MOBI_MAGIC) {
        return None;
    }
    big_endian(source, header + VERSION_AT)
}

fn big_endian(source: &impl Bytes, at: u64) -> Option<u32> {
    let bytes = source.slice(at, 4).ok()?;
    (bytes.len() == 4).then(|| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Palm database holding one record with a MOBI header of `version`.
    fn book(version: Option<u32>) -> Vec<u8> {
        let record_at: u32 = 1024;
        let mut out = vec![0u8; record_at as usize];
        out[TYPE_AT as usize..TYPE_AT as usize + 8].copy_from_slice(BOOK_TYPE);
        out[FIRST_RECORD_AT as usize..FIRST_RECORD_AT as usize + 4]
            .copy_from_slice(&record_at.to_be_bytes());

        if let Some(version) = version {
            out.resize(record_at as usize + MOBI_HEADER_AT as usize, 0);
            out.extend_from_slice(MOBI_MAGIC);
            out.resize(
                record_at as usize + MOBI_HEADER_AT as usize + VERSION_AT as usize,
                0,
            );
            out.extend_from_slice(&version.to_be_bytes());
        }
        out
    }

    #[test]
    fn the_newer_format_is_recognised() {
        assert_eq!(kind(&book(Some(8)).as_slice()), Some(Kind::Azw3));
        assert_eq!(kind(&book(Some(9)).as_slice()), Some(Kind::Azw3));
    }

    #[test]
    fn the_older_format_is_recognised() {
        assert_eq!(kind(&book(Some(6)).as_slice()), Some(Kind::Mobi));
    }

    #[test]
    fn a_book_with_no_readable_version_is_taken_for_the_older_format() {
        assert_eq!(kind(&book(None).as_slice()), Some(Kind::Mobi));
    }

    #[test]
    fn something_that_is_not_a_kindle_book_is_not_one() {
        assert_eq!(kind(&b"%PDF-1.7".as_slice()), None);
        assert_eq!(kind(&vec![0u8; 4096].as_slice()), None);
    }

    #[test]
    fn a_truncated_book_does_not_panic() {
        let whole = book(Some(8));
        for cut in 0..whole.len() {
            let _ = kind(&&whole[..cut]);
        }
    }
}
