//! Telling a FictionBook from any other XML.
//!
//! A FictionBook is an XML document whose root element is `FictionBook`.
//! The root comes after the prolog — a declaration, comments, perhaps a
//! doctype — so it is found by stepping over those rather than by matching
//! at a fixed offset.
//!
//! The walk is over bytes, not text. A great many FictionBooks are written in
//! windows-1251, which is not UTF-8 but spells its markup in ASCII all the
//! same, and a reader that insisted on UTF-8 would call every one of them
//! binary.

use crate::BOM;

/// What the root element of a FictionBook is called.
const ROOT: &[u8] = b"FictionBook";

/// Whether the front of a file is the start of a FictionBook.
///
/// `head` is however much of the front the caller read. A root element that
/// does not begin inside it is not found, so the answer is no rather than a
/// guess.
pub fn is_fictionbook(head: &[u8]) -> bool {
    root_element(head).is_some_and(|name| local_name(name) == ROOT)
}

/// The name of the first element, after the prolog.
fn root_element(head: &[u8]) -> Option<&[u8]> {
    let mut rest = head.strip_prefix(BOM).unwrap_or(head);
    loop {
        rest = rest.trim_ascii_start();
        if let Some(after) = rest.strip_prefix(b"<?") {
            rest = after_marker(after, b"?>")?;
        } else if let Some(after) = rest.strip_prefix(b"<!--") {
            rest = after_marker(after, b"-->")?;
        } else if let Some(after) = rest.strip_prefix(b"<!") {
            rest = after_declaration(after)?;
        } else {
            // Anything but an element here is text before the root, which
            // XML does not allow.
            let after = rest.strip_prefix(b"<")?;
            // A name that runs to the end of what was read may be the front
            // of a longer one, so it is not taken.
            let end = after.iter().position(|byte| !is_name_byte(*byte))?;
            let ends_properly =
                matches!(after[end], b'>' | b'/') || after[end].is_ascii_whitespace();
            return (end > 0 && ends_properly).then_some(&after[..end]);
        }
    }
}

/// What follows the first `marker`, if it is there at all.
fn after_marker<'a>(rest: &'a [u8], marker: &[u8]) -> Option<&'a [u8]> {
    rest.windows(marker.len())
        .position(|window| window == marker)
        .map(|at| &rest[at + marker.len()..])
}

/// What follows a doctype, whose internal subset may hold `>` of its own.
fn after_declaration(rest: &[u8]) -> Option<&[u8]> {
    let mut depth = 0usize;
    let mut quote = None;
    for (at, &byte) in rest.iter().enumerate() {
        match quote {
            Some(open) if byte == open => quote = None,
            Some(_) => {}
            None => match byte {
                b'"' | b'\'' => quote = Some(byte),
                b'[' => depth += 1,
                b']' => depth = depth.saturating_sub(1),
                b'>' if depth == 0 => return Some(&rest[at + 1..]),
                _ => {}
            },
        }
    }
    None
}

/// The name without its namespace prefix.
fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|byte| *byte == b':') {
        Some(at) => &name[at + 1..],
        None => name,
    }
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':') || byte >= 0x80
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOK: &[u8] = b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
        <FictionBook xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\">\n\
        <description><title-info><book-title>The Mill</book-title></title-info></description>";

    #[test]
    fn a_fictionbook_is_recognised() {
        assert!(is_fictionbook(BOOK));
        assert!(is_fictionbook(b"<FictionBook>"));
    }

    #[test]
    fn the_prolog_is_stepped_over() {
        let head = b"\xef\xbb\xbf  <?xml version=\"1.0\"?>\r\n\
            <!-- converted by hand -->\n\
            <!DOCTYPE FictionBook [ <!ENTITY mill \"<b>the mill</b>\"> ]>\n\
            <?xml-stylesheet href=\"fb2.css\"?>\n\
            <FictionBook\n xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\">";
        assert!(is_fictionbook(head));
    }

    #[test]
    fn a_prefixed_root_is_still_a_fictionbook() {
        assert!(is_fictionbook(
            b"<fb:FictionBook xmlns:fb=\"http://www.gribuser.ru/xml/fictionbook/2.0\">"
        ));
    }

    #[test]
    fn markup_need_not_be_utf8() {
        // windows-1251 for "Мельница" in the title.
        let head = b"<?xml version=\"1.0\" encoding=\"windows-1251\"?>\
            <FictionBook><description><book-title>\xcc\xe5\xeb\xfc\xed\xe8\xf6\xe0</book-title>";
        assert!(is_fictionbook(head));
    }

    #[test]
    fn other_xml_is_not_a_fictionbook() {
        assert!(!is_fictionbook(
            b"<?xml version=\"1.0\"?>\n<rss version=\"2.0\">"
        ));
        assert!(!is_fictionbook(b"<?xml version=\"1.0\"?><FictionBookish>"));
        assert!(!is_fictionbook(b"<html><FictionBook>"));
        assert!(!is_fictionbook(b"text before <FictionBook>"));
        assert!(!is_fictionbook(b""));
    }

    #[test]
    fn a_root_the_window_cut_short_is_not_taken() {
        assert!(!is_fictionbook(b"<?xml version=\"1.0\"?><FictionBook"));
        assert!(!is_fictionbook(b"<?xml version=\"1.0\"?><Fiction"));
        assert!(!is_fictionbook(b"<?xml version=\"1.0\""));
        assert!(!is_fictionbook(b"<!-- never closed <FictionBook>"));
    }

    #[test]
    fn a_truncated_or_mangled_prolog_does_not_panic() {
        let head = b"\xef\xbb\xbf<?xml?><!--c--><!DOCTYPE x [ <!ENTITY a '>'> ] \"]\">\
            <FictionBook>";
        assert!(is_fictionbook(head));
        for cut in 0..head.len() {
            let _ = is_fictionbook(&head[..cut]);
        }
        for byte in 0..=255u8 {
            let _ = is_fictionbook(&[b'<', byte]);
            let _ = is_fictionbook(&[b'<', b'!', byte, b'>']);
        }
    }
}
