//! Packing paragraphs into chunks of about the size an embedding model wants.
//!
//! Page first, paragraph second, sentence only when a paragraph is itself too
//! big. Every flush carries a tail of the previous chunk forward, so a
//! sentence that answers a question is not orphaned from the sentence that
//! asked it.

/// The token budgets a chunker works to.
///
/// The defaults suit a model with a 512-token comfortable window. A model
/// with a larger one wants roughly double everything — the numbers below are
/// ratios that have held up, not constants with a life of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budgets {
    /// Where a chunk is flushed, in tokens.
    pub target: usize,
    /// How much of the previous chunk is carried into the next one.
    pub overlap: usize,
    /// Chunks smaller than this are dropped: a fragment retrieves badly and
    /// costs an embedding.
    pub min_tokens: usize,
    /// No chunk may exceed this, whatever structure says.
    pub hard_max: usize,
}

/// A passage as packed, and where in the page it began.
///
/// The offset is what lets a chunk be placed back in the document it came
/// from — which is how it finds the heading it sits under. Without it every
/// chunk of an unpaginated document answers with the first heading in the
/// file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packed {
    /// The passage.
    pub text: String,
    /// Byte offset within the page's text where the passage begins.
    pub offset: usize,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            target: 512,
            overlap: 64,
            min_tokens: 50,
            hard_max: 768,
        }
    }
}

/// Pack one page's text into chunk-sized pieces.
pub fn page(text: &str, budgets: &Budgets) -> Vec<Packed> {
    let paragraphs = textkit::paragraphs(text);
    if paragraphs.is_empty() {
        return Vec::new();
    }
    // Where a piece began, so that what is packed can still be placed. The
    // pieces are slices of `text`, so the arithmetic is exact.
    let at = |piece: &str| (piece.as_ptr() as usize).saturating_sub(text.as_ptr() as usize);

    let mut chunks: Vec<Packed> = Vec::new();
    let mut buf: Vec<(String, usize)> = Vec::new();
    let mut buf_tokens = 0usize;

    for paragraph in paragraphs {
        let paragraph_tokens = textkit::count(paragraph);

        if paragraph_tokens > budgets.hard_max {
            // A paragraph nothing can hold: break it at sentences instead.
            // Note that a flush from here joins with a space, while the final
            // flush below joins with a blank line — the seam a chunk was cut
            // at is visible in how its pieces are rejoined.
            for sentence in textkit::sentences(paragraph) {
                let sentence_tokens = textkit::count(sentence);
                if buf_tokens + sentence_tokens > budgets.target && !buf.is_empty() {
                    chunks.push(joined(&buf, " "));
                    buf = carry_tail(&buf, budgets.overlap);
                    buf_tokens = textkit::count(&joined(&buf, " ").text);
                }
                buf.push((sentence.to_string(), at(sentence)));
                buf_tokens += sentence_tokens;
            }
            continue;
        }

        if buf_tokens + paragraph_tokens > budgets.target && !buf.is_empty() {
            chunks.push(joined(&buf, "\n\n"));
            buf = carry_tail(&buf, budgets.overlap);
            buf_tokens = textkit::count(&joined(&buf, "\n\n").text);
        }
        buf.push((paragraph.to_string(), at(paragraph)));
        buf_tokens += paragraph_tokens;
    }

    if !buf.is_empty() {
        chunks.push(joined(&buf, "\n\n"));
    }
    chunks
}

/// Join a buffer's pieces, keeping where the first of them began.
fn joined(buf: &[(String, usize)], separator: &str) -> Packed {
    Packed {
        text: buf
            .iter()
            .map(|(piece, _)| piece.as_str())
            .collect::<Vec<_>>()
            .join(separator),
        offset: buf.first().map(|(_, at)| *at).unwrap_or_default(),
    }
}

/// Keep the last pieces of a buffer, up to `overlap` tokens of context.
///
/// Always keeps at least one piece, even when that piece alone is over
/// budget: an overlap of nothing is a chunk boundary with no bridge across
/// it, which is the thing overlap exists to prevent.
fn carry_tail(buf: &[(String, usize)], overlap: usize) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    let mut total = 0usize;
    for piece in buf.iter().rev() {
        let tokens = textkit::count(&piece.0);
        if total + tokens > overlap && !out.is_empty() {
            break;
        }
        out.insert(0, piece.clone());
        total += tokens;
    }
    out
}

/// Split anything still over the hard maximum, on token boundaries, and hand
/// back each piece with its token count.
///
/// Defence in depth. The structural passes above should have handled it; this
/// is what guarantees the promise regardless. The counts come back with the
/// pieces because they have just been computed, and counting them again is
/// the second-largest cost in the whole chunker.
pub fn enforce_hard_max(text: &str, hard_max: usize) -> Vec<(String, usize)> {
    let tokens = textkit::count(text);
    if tokens <= hard_max {
        return vec![(text.to_string(), tokens)];
    }
    textkit::split_at_token_limit(text, hard_max)
        .into_iter()
        .map(|piece| {
            let tokens = textkit::count(&piece);
            (piece, tokens)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(packed: Vec<Packed>) -> Vec<String> {
        packed.into_iter().map(|one| one.text).collect()
    }

    fn budgets(target: usize, overlap: usize) -> Budgets {
        Budgets {
            target,
            overlap,
            ..Budgets::default()
        }
    }

    #[test]
    fn short_text_is_one_chunk() {
        let chunks = texts(page("one\n\ntwo", &Budgets::default()));
        assert_eq!(chunks, ["one\n\ntwo"]);
    }

    #[test]
    fn paragraphs_are_rejoined_with_a_blank_line() {
        let chunks = texts(page("alpha\n\n\n\nbeta", &Budgets::default()));
        assert_eq!(chunks, ["alpha\n\nbeta"]);
    }

    #[test]
    fn a_full_buffer_flushes_and_carries_a_tail() {
        let text = "one two three\n\nfour five six\n\nseven eight nine";
        let chunks = texts(page(text, &budgets(6, 4)));
        assert!(chunks.len() > 1, "{chunks:?}");
        // Every chunk after the first repeats the end of the one before it.
        for pair in chunks.windows(2) {
            let earlier_tail = pair[0].rsplit("\n\n").next().expect("a paragraph");
            assert!(
                pair[1].starts_with(earlier_tail),
                "{:?} does not carry {:?}",
                pair[1],
                earlier_tail
            );
        }
    }

    #[test]
    fn an_oversized_paragraph_is_cut_at_sentences() {
        let paragraph = "Alpha one. Beta two. Gamma three. Delta four. Epsilon five.";
        let chunks = texts(page(
            paragraph,
            &Budgets {
                target: 6,
                overlap: 2,
                min_tokens: 1,
                hard_max: 8,
            },
        ));
        assert!(chunks.len() > 1, "{chunks:?}");
        assert!(chunks[0].contains(' '));
    }

    #[test]
    fn an_empty_page_packs_to_nothing() {
        assert!(page("   \n\n  ", &Budgets::default()).is_empty());
    }

    #[test]
    fn the_tail_keeps_one_piece_even_when_it_is_over_budget() {
        let buf = vec![("a longer paragraph than the overlap allows".to_string(), 0)];
        assert_eq!(carry_tail(&buf, 1).len(), 1);
    }

    #[test]
    fn nothing_leaves_the_packer_over_the_hard_maximum() {
        let long = "word ".repeat(2000);
        for (piece, tokens) in enforce_hard_max(&long, 100) {
            assert!(tokens <= 100);
            assert_eq!(tokens, textkit::count(&piece));
        }
    }
}
