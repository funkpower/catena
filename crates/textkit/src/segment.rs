//! Cutting text into sentences and paragraphs.
//!
//! Both segmenters reproduce what Python's `re` module does with
//! `(?<=[.!?])\s+` and `\n\s*\n`, because the chunk boundaries already in
//! existing indexes were cut that way. Where the obvious Rust spelling would
//! differ, the difference is spelled out below rather than smoothed over.

/// Whether a character is whitespace, by Python's definition.
///
/// Python's `str.isspace` and its `\s` class cover the Unicode `White_Space`
/// property *and* the four ASCII separators `U+001C`–`U+001F`, which Rust's
/// [`char::is_whitespace`] does not. Trimming with the narrower definition
/// leaves a separator on the front of a piece, which changes its token count,
/// which moves a chunk boundary.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}

/// Trim leading and trailing whitespace, by Python's definition.
fn trim(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// Drop the empty pieces and trim what is left, which is what every caller of
/// a Python `re.split` here does immediately afterwards.
fn tidy(pieces: Vec<&str>) -> Vec<&str> {
    pieces
        .into_iter()
        .map(trim)
        .filter(|piece| !piece.is_empty())
        .collect()
}

/// Split text into sentences at whitespace that follows `.`, `!` or `?`.
///
/// Crude on purpose — it does not know that `Mr.` or `p. 41` are not
/// sentence ends. This is the fallback path for a paragraph too large to
/// chunk whole, where a wrong cut costs a little context and a clever cut
/// that disagrees with the existing index costs a re-embedding.
pub fn sentences(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut piece_start = 0;
    let mut previous = None;
    let mut chars = text.char_indices().peekable();

    while let Some((at, c)) = chars.next() {
        if !is_space(c) {
            previous = Some(c);
            continue;
        }
        // Whitespace is consumed a whole run at a time, because `\s+` is
        // greedy: the separator is the run, not its first character.
        let mut run_end = at + c.len_utf8();
        while let Some(&(next_at, next)) = chars.peek() {
            if !is_space(next) {
                break;
            }
            run_end = next_at + next.len_utf8();
            chars.next();
        }
        if matches!(previous, Some('.' | '!' | '?')) {
            pieces.push(&text[piece_start..at]);
            piece_start = run_end;
        }
        previous = None;
    }

    pieces.push(&text[piece_start..]);
    tidy(pieces)
}

/// Split text into paragraphs at blank lines.
///
/// A separator is a newline, any run of whitespace, and a newline — so one
/// blank line and four of them separate equally, and trailing spaces on an
/// otherwise blank line do not stop it being blank.
pub fn paragraphs(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut piece_start = 0;
    let mut chars = text.char_indices().peekable();

    while let Some((at, c)) = chars.next() {
        if c != '\n' {
            continue;
        }
        // The greedy `\s*` in `\n\s*\n` backtracks to the *last* newline in
        // the whitespace run, so that is where the next paragraph begins.
        let mut last_newline = None;
        while let Some(&(next_at, next)) = chars.peek() {
            if !is_space(next) {
                break;
            }
            if next == '\n' {
                last_newline = Some(next_at);
            }
            chars.next();
        }
        if let Some(newline) = last_newline {
            pieces.push(&text[piece_start..at]);
            piece_start = newline + 1;
        }
    }

    pieces.push(&text[piece_start..]);
    tidy(pieces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_split_after_terminators() {
        assert_eq!(
            sentences("One. Two! Three? Four"),
            ["One.", "Two!", "Three?", "Four"]
        );
    }

    #[test]
    fn a_terminator_without_following_whitespace_is_not_a_boundary() {
        assert_eq!(sentences("etc.and on"), ["etc.and on"]);
        assert_eq!(sentences("3.14 is pi"), ["3.14 is pi"]);
    }

    #[test]
    fn the_whole_whitespace_run_is_the_separator() {
        assert_eq!(sentences("One.\n\n   Two."), ["One.", "Two."]);
    }

    #[test]
    fn leading_whitespace_is_not_a_boundary() {
        assert_eq!(sentences("   One. Two."), ["One.", "Two."]);
    }

    #[test]
    fn empty_and_blank_text_yield_nothing() {
        assert!(sentences("").is_empty());
        assert!(sentences("   \n  ").is_empty());
        assert!(paragraphs("").is_empty());
        assert!(paragraphs("\n\n\n").is_empty());
    }

    #[test]
    fn paragraphs_split_on_a_blank_line() {
        assert_eq!(paragraphs("one\n\ntwo"), ["one", "two"]);
    }

    #[test]
    fn a_single_newline_is_not_a_paragraph_break() {
        assert_eq!(paragraphs("one\ntwo"), ["one\ntwo"]);
    }

    #[test]
    fn a_blank_line_of_spaces_still_separates() {
        assert_eq!(paragraphs("one\n \t \ntwo"), ["one", "two"]);
    }

    #[test]
    fn many_blank_lines_separate_once() {
        assert_eq!(paragraphs("one\n\n\n\n  two"), ["one", "two"]);
    }

    #[test]
    fn carriage_returns_survive_the_split() {
        assert_eq!(paragraphs("one\r\n\r\ntwo"), ["one", "two"]);
    }

    #[test]
    fn ascii_separators_count_as_whitespace() {
        // Python's str.isspace is true for these; Rust's is_whitespace is not.
        assert!(is_space('\u{1c}'));
        assert_eq!(sentences("One.\u{1c}Two."), ["One.", "Two."]);
    }

    #[test]
    fn multi_byte_characters_do_not_shift_the_boundaries() {
        assert_eq!(sentences("Café. Naïve."), ["Café.", "Naïve."]);
        assert_eq!(paragraphs("Café\n\nNaïve"), ["Café", "Naïve"]);
    }
}
