//! Character classes that mean in Rust what they mean in Python.
//!
//! The two regex engines disagree about `\s`, `\w` and therefore `\b`, and
//! every disagreement is a heuristic that fires on one side and not the
//! other. The differences are small and entirely real:
//!
//! - Python's `\s` also matches `U+001C`–`U+001F`, the four ASCII
//!   information separators.
//! - Python's `\w` is the letters, the numbers and the underscore. Rust's
//!   adds every combining mark, every connector punctuation mark and the
//!   join controls, and leaves out the letter-like and other numbers — so
//!   `Gödel` written in decomposed form is one word to Rust and two to
//!   Python, and `²` is a word character to Python and not to Rust.
//! - `\b` is defined in terms of `\w`, so it inherits all of that.
//!
//! Spell a ported pattern with these and it means the same thing on both
//! sides. The boundary forms are lookarounds, so a pattern using them needs
//! an engine that has them.

/// Python's `\s`.
pub const SPACE: &str = r"[\s\x{1c}-\x{1f}]";

/// Python's `\w`.
pub const WORD: &str = r"[\p{L}\p{N}_]";

/// Python's `\b`, before a word.
pub const WORD_START: &str = r"(?<![\p{L}\p{N}_])";

/// Python's `\b`, after a word.
pub const WORD_END: &str = r"(?![\p{L}\p{N}_])";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_space_class_covers_what_python_trims() {
        let space = regex::Regex::new(SPACE).expect("a valid pattern");
        for c in [' ', '\t', '\n', '\u{a0}', '\u{1c}', '\u{1f}', '\u{2028}'] {
            assert!(space.is_match(&c.to_string()), "{c:?}");
            assert!(crate::is_space(c), "{c:?}");
        }
        for c in ['a', '-', '\u{200b}'] {
            assert!(!space.is_match(&c.to_string()), "{c:?}");
        }
    }

    #[test]
    fn the_word_class_is_letters_numbers_and_underscore() {
        let word = regex::Regex::new(&format!("^{WORD}$")).expect("a valid pattern");
        for c in ['a', 'Z', '0', '_', 'é', 'Ω', '²', 'Ⅷ'] {
            assert!(word.is_match(&c.to_string()), "{c:?}");
        }
        // The combining marks Rust's own `\w` would accept.
        for c in ['\u{301}', '\u{363}', '-', ' '] {
            assert!(!word.is_match(&c.to_string()), "{c:?}");
        }
    }
}
