//! Token counting, against the same tokeniser the embedding stack uses.

use tiktoken_rs::{CoreBPE, cl100k_base_singleton};

/// The tokeniser every tool in this workspace counts with.
///
/// `cl100k_base`, loaded from data compiled into the binary — no download, no
/// cache directory, nothing to go stale. Counting against a different
/// tokeniser than the one the index was built with silently moves every chunk
/// boundary, which is why this is one function and not a parameter.
fn bpe() -> &'static CoreBPE {
    cl100k_base_singleton()
}

/// Token ids for `text`, with special-token sequences treated as ordinary
/// text rather than as control tokens.
///
/// Text out of a PDF can contain anything, including the literal string
/// `<|endoftext|>`; encoding it as a control token would be both wrong and a
/// way for a document to reach into the model's input.
pub fn encode(text: &str) -> Vec<u32> {
    bpe().encode_ordinary(text)
}

/// How many tokens `text` is.
pub fn count(text: &str) -> usize {
    encode(text).len()
}

/// Text for `tokens`, with undecodable byte sequences replaced.
///
/// Cutting a token list at an arbitrary index can split a multi-byte
/// character, so decoding a slice has to be lossy or it would fail on exactly
/// the inputs that need it.
pub fn decode_lossy(tokens: &[u32]) -> String {
    match bpe().decode_bytes(tokens) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    }
}

/// Split `text` into pieces of at most `limit` tokens, cutting on token
/// boundaries.
///
/// The last resort of a chunker: it guarantees a maximum but respects no
/// other structure, so it should only ever see text that the structural
/// passes could not get under the limit. Returns `text` unchanged when it
/// already fits. A `limit` of zero yields nothing, since no piece could
/// satisfy it.
pub fn split_at_token_limit(text: &str, limit: usize) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }
    let ids = encode(text);
    if ids.len() <= limit {
        return vec![text.to_string()];
    }
    ids.chunks(limit).map(decode_lossy).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_agree_with_the_python_tokeniser() {
        // Values from `tiktoken.get_encoding("cl100k_base").encode(s,
        // disallowed_special=())`.
        for (text, expected) in [
            ("", 0),
            ("hello world", 2),
            ("The quick brown fox jumps over the lazy dog.", 10),
            ("\n\n", 1),
            ("naïve café", 4),
        ] {
            assert_eq!(count(text), expected, "counting {text:?}");
        }
    }

    #[test]
    fn a_special_token_spelling_is_just_text() {
        // Seven ordinary tokens, not the one control token it names.
        assert_eq!(count("<|endoftext|>"), 7);
    }

    #[test]
    fn text_under_the_limit_comes_back_untouched() {
        assert_eq!(split_at_token_limit("hello world", 10), ["hello world"]);
    }

    #[test]
    fn splitting_preserves_every_character() {
        let text = "The quick brown fox jumps over the lazy dog, repeatedly and at length.";
        let pieces = split_at_token_limit(text, 3);
        assert!(pieces.len() > 1);
        assert_eq!(pieces.concat(), text);
    }

    #[test]
    fn no_piece_exceeds_the_limit() {
        let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu";
        for piece in split_at_token_limit(text, 4) {
            assert!(count(&piece) <= 4, "piece {piece:?} is over the limit");
        }
    }

    #[test]
    fn a_zero_limit_yields_nothing() {
        assert!(split_at_token_limit("anything", 0).is_empty());
    }

    #[test]
    fn splitting_a_multi_byte_character_is_lossy_not_fatal() {
        // Every emoji is several tokens, so a limit of one cuts inside one.
        let pieces = split_at_token_limit("🙂🙂🙂", 1);
        assert!(!pieces.is_empty());
    }
}
