//! Listing what is inside a zip, without unpacking any of it.
//!
//! An EPUB, an htmlz and a PDF someone zipped and renamed are all zips, and
//! telling them apart needs only the member names. Those live in the central
//! directory at the end of the file, so reading them costs a few kilobytes
//! whatever the archive weighs — and nothing is ever decompressed, which
//! means a malicious archive cannot expand into anything here.

use crate::bytes::Bytes;

/// The end-of-central-directory record's signature.
const END_OF_DIRECTORY: &[u8] = b"PK\x05\x06";
/// A central directory entry's signature.
const ENTRY: &[u8] = b"PK\x01\x02";
/// A local file header, which is what a zip starts with.
pub const LOCAL_HEADER: &[u8] = b"PK\x03\x04";

/// How far back from the end the trailing record may be.
///
/// Its own size plus the largest comment a zip may carry.
const SEARCH_BACK: u64 = 22 + 0xffff;

/// Most members this will list.
///
/// A book in an archive has tens of members; an archive with a hundred
/// thousand is not a book, and reading all of its names would be the only
/// expensive thing this program does.
const MOST_MEMBERS: usize = 100_000;

/// The names of an archive's members, or `None` if it is not a zip this can
/// read.
pub fn members(source: &impl Bytes) -> Option<Vec<String>> {
    if !source.magic_at(0, LOCAL_HEADER) {
        return None;
    }
    let (count, mut at) = directory_start(source)?;

    let mut names = Vec::with_capacity(count.min(1024));
    for _ in 0..count.min(MOST_MEMBERS) {
        let header = source.slice(at, 46).ok()?;
        if header.len() < 46 || header[..4] != *ENTRY {
            break;
        }
        let name_len = u16::from_le_bytes([header[28], header[29]]) as u64;
        let extra_len = u16::from_le_bytes([header[30], header[31]]) as u64;
        let comment_len = u16::from_le_bytes([header[32], header[33]]) as u64;

        let name = source.slice(at + 46, name_len as usize).ok()?;
        // Member names are bytes, and an archive written on another machine
        // may not have written UTF-8. What is wanted here is a comparison
        // against ASCII, so anything else may as well be a replacement
        // character.
        names.push(String::from_utf8_lossy(&name).into_owned());
        at += 46 + name_len + extra_len + comment_len;
    }
    Some(names)
}

/// How many entries the central directory has and where it starts.
fn directory_start(source: &impl Bytes) -> Option<(usize, u64)> {
    let len = source.len();
    let window = SEARCH_BACK.min(len);
    let from = len - window;
    let tail = source.slice(from, window as usize).ok()?;

    // The record is at the end, after a comment of unknown length, so it is
    // found by searching backwards for its signature.
    let at = tail
        .windows(4)
        .rposition(|window| window == END_OF_DIRECTORY)?;
    let record = &tail[at..];
    if record.len() < 22 {
        return None;
    }
    let count = u16::from_le_bytes([record[10], record[11]]) as usize;
    let offset = u32::from_le_bytes([record[16], record[17], record[18], record[19]]) as u64;
    (offset < len).then_some((count, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a zip holding `names`, each with empty stored content.
    fn zip(names: &[&str]) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        let mut offsets = Vec::new();
        for name in names {
            offsets.push(out.len() as u32);
            out.extend_from_slice(LOCAL_HEADER);
            out.extend_from_slice(&[0; 22]); // version .. uncompressed size
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
        }
        let directory_at = out.len() as u32;
        for (name, offset) in names.iter().zip(&offsets) {
            out.extend_from_slice(ENTRY);
            out.extend_from_slice(&[0; 24]); // version .. uncompressed size
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // extra
            out.extend_from_slice(&0u16.to_le_bytes()); // comment
            out.extend_from_slice(&[0; 8]); // disk, attributes
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
        }
        out.extend_from_slice(END_OF_DIRECTORY);
        out.extend_from_slice(&[0; 4]); // disk numbers
        out.extend_from_slice(&(names.len() as u16).to_le_bytes());
        out.extend_from_slice(&(names.len() as u16).to_le_bytes());
        out.extend_from_slice(&((out.len() as u32).saturating_sub(directory_at)).to_le_bytes());
        out.extend_from_slice(&directory_at.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // comment length
        out
    }

    #[test]
    fn the_member_names_come_back() {
        let archive = zip(&["mimetype", "META-INF/container.xml", "OEBPS/book.opf"]);
        let names = members(&archive.as_slice()).expect("a zip");
        assert_eq!(
            names,
            ["mimetype", "META-INF/container.xml", "OEBPS/book.opf"]
        );
    }

    #[test]
    fn an_archive_with_no_members_is_not_read_as_one() {
        // It has no local header to start with, so nothing identifies it as
        // an archive in the first place — and an empty archive is not a book
        // under any reading.
        assert_eq!(members(&zip(&[]).as_slice()), None);
    }

    #[test]
    fn something_that_is_not_a_zip_is_not_read_as_one() {
        assert_eq!(members(&b"%PDF-1.7".as_slice()), None);
        assert_eq!(members(&b"".as_slice()), None);
    }

    #[test]
    fn a_truncated_archive_does_not_panic() {
        let archive = zip(&["a.pdf", "b.pdf"]);
        for cut in 0..archive.len() {
            let _ = members(&&archive[..cut]);
        }
    }

    #[test]
    fn a_count_that_lies_stops_at_the_first_bad_entry() {
        let mut archive = zip(&["a.pdf"]);
        let at = archive.len() - 12;
        // Claim a thousand entries where there is one.
        archive[at - 6..at - 4].copy_from_slice(&1000u16.to_le_bytes());
        let names = members(&archive.as_slice()).expect("a zip");
        assert_eq!(names, ["a.pdf"]);
    }
}
