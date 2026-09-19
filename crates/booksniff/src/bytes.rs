//! Reading a few bytes of a file without reading the file.
//!
//! Sniffing a format needs the first few bytes, and sometimes a structure at
//! the end. A nine-hundred-megabyte scan should cost neither nine hundred
//! megabytes of memory nor nine hundred megabytes of reading, so everything
//! here works through a window rather than a buffer — and the same trait is
//! implemented for a slice, so the tests need no files at all.

use std::io::{Read, Seek, SeekFrom};

/// Something a few bytes can be read out of, at a position.
pub trait Bytes {
    /// How long the whole thing is.
    fn len(&self) -> u64;

    /// Whether there is nothing at all.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Read up to `into.len()` bytes from `offset`, returning how many were
    /// read. Short at the end, zero past it.
    fn read_at(&self, offset: u64, into: &mut [u8]) -> std::io::Result<usize>;

    /// Read exactly `count` bytes from `offset`, or as many as there are.
    fn slice(&self, offset: u64, count: usize) -> std::io::Result<Vec<u8>> {
        let mut buffer = vec![0; count];
        let read = self.read_at(offset, &mut buffer)?;
        buffer.truncate(read);
        Ok(buffer)
    }

    /// Whether the bytes at `offset` are exactly `magic`.
    fn magic_at(&self, offset: u64, magic: &[u8]) -> bool {
        self.slice(offset, magic.len())
            .is_ok_and(|read| read == magic)
    }
}

impl Bytes for &[u8] {
    fn len(&self) -> u64 {
        <[u8]>::len(self) as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> std::io::Result<usize> {
        let Ok(offset) = usize::try_from(offset) else {
            return Ok(0);
        };
        if offset >= <[u8]>::len(self) {
            return Ok(0);
        }
        let available = &self[offset..];
        let take = available.len().min(into.len());
        into[..take].copy_from_slice(&available[..take]);
        Ok(take)
    }
}

/// A file on disk, read through a window.
pub struct File {
    file: std::cell::RefCell<std::fs::File>,
    len: u64,
}

impl File {
    /// Open a file for sniffing.
    pub fn open(path: &std::path::Path) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self {
            file: std::cell::RefCell::new(file),
            len,
        })
    }
}

impl Bytes for File {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> std::io::Result<usize> {
        let mut file = self.file.borrow_mut();
        file.seek(SeekFrom::Start(offset))?;
        let mut read = 0;
        while read < into.len() {
            match file.read(&mut into[read..])? {
                0 => break,
                n => read += n,
            }
        }
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_reads_from_the_middle() {
        let bytes: &[u8] = b"hello world";
        assert_eq!(bytes.slice(6, 5).unwrap(), b"world");
        assert_eq!(Bytes::len(&bytes), 11);
    }

    #[test]
    fn reading_past_the_end_is_short_rather_than_an_error() {
        let bytes: &[u8] = b"abc";
        assert_eq!(bytes.slice(1, 99).unwrap(), b"bc");
        assert!(bytes.slice(99, 4).unwrap().is_empty());
        assert!(bytes.slice(u64::MAX, 4).unwrap().is_empty());
    }

    #[test]
    fn magic_matches_only_when_it_is_all_there() {
        let bytes: &[u8] = b"%PDF-1.7";
        assert!(bytes.magic_at(0, b"%PDF-"));
        assert!(!bytes.magic_at(0, b"%PDG-"));
        assert!(!bytes.magic_at(4, b"-1.7 more"));
    }

    #[test]
    fn a_file_reads_the_same_as_a_slice() {
        let path = std::env::temp_dir().join("booksniff-bytes-test");
        std::fs::write(&path, b"hello world").expect("a writable path");
        let file = File::open(&path).expect("the file opens");
        assert_eq!(file.len(), 11);
        assert_eq!(file.slice(6, 5).unwrap(), b"world");
        assert!(file.magic_at(0, b"hello"));
    }
}
