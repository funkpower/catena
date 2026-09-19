//! Cuts markdown into retrievable passages that carry their page, chapter
//! and role.
//!
//! The order of the passes is load-bearing and is not obvious:
//!
//! 1. Split on page markers, so a chunk never spans a page and its page
//!    number is exact rather than inferred.
//! 2. Pack paragraphs toward a token target within each page, carrying an
//!    overlap forward.
//! 3. Enforce the hard maximum on token boundaries, whatever the structure
//!    said.
//! 4. Drop what is too small to be worth an embedding.
//! 5. Flag the note chunks, *before* the structural marker is stripped out
//!    of their text.
//! 6. Classify roles across the whole document at once, because smoothing a
//!    stray chunk needs to see its neighbours.

pub mod footnote;
pub mod pack;
pub mod pages;
pub mod role;

use corpus_core::{Chunk, chunk::flags};

pub use pack::Budgets;

/// Cut `markdown` into chunks.
pub fn chunk(markdown: &str, budgets: &Budgets) -> Vec<Chunk> {
    let pages = pages::split_by_page(markdown);
    let chapters = pages::chapters(markdown);
    let by_offset = pages::ChapterIndex::new(&chapters);

    let mut chunks: Vec<Chunk> = Vec::new();
    for page in pages {
        for packed in pack::page(page.text, budgets) {
            for (piece, tokens) in pack::enforce_hard_max(&packed, budgets.hard_max) {
                if tokens < budgets.min_tokens {
                    continue;
                }
                // A page-based chapter when the document has page markers;
                // the byte offset otherwise, which is what carries EPUBs
                // that have headings but no page anchors. Page zero counts as
                // no page at all, which is what the Python's truth test makes
                // of it.
                let chapter = page
                    .number
                    .filter(|number| *number != 0)
                    .and_then(|number| pages::chapter_of_page(&chapters, number))
                    .or_else(|| by_offset.at(page.offset));

                let idx = chunks.len() as u32;
                let notes = footnote::looks_like_citations(&piece);
                let mut chunk = Chunk::new(idx, footnote::strip_sentinel(&piece), tokens as u32);
                chunk.page = page.number;
                chunk.chapter = chapter.map(str::to_string);
                if notes {
                    chunk.flag(flags::FOOTNOTE_CITATIONS);
                }
                chunks.push(chunk);
            }
        }
    }

    let mut roles: Vec<_> = chunks
        .iter()
        .map(|chunk| role::classify(&chunk.text, chunk.chapter.as_deref()))
        .collect();
    role::smooth(&mut roles);
    for (chunk, role) in chunks.iter_mut().zip(roles) {
        chunk.role = role;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use corpus_core::Role;

    fn small() -> Budgets {
        Budgets {
            target: 20,
            overlap: 4,
            min_tokens: 1,
            hard_max: 40,
        }
    }

    #[test]
    fn chunks_are_numbered_from_zero_without_gaps() {
        let markdown = "# One\n\nalpha beta\n\n<!-- Page 2 -->\n\ngamma delta\n\nepsilon zeta";
        let chunks = chunk(markdown, &small());
        for (at, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.idx, at as u32);
        }
    }

    #[test]
    fn a_chunk_carries_the_page_it_came_from() {
        let markdown = "<!-- Page 7 -->\n\nalpha beta gamma";
        let chunks = chunk(markdown, &small());
        assert_eq!(chunks[0].page, Some(7));
    }

    #[test]
    fn a_chunk_carries_its_chapter() {
        let markdown = "<!-- Page 1 -->\n\n# The Second Estate\n\nalpha beta gamma";
        let chunks = chunk(markdown, &small());
        assert_eq!(chunks[0].chapter.as_deref(), Some("The Second Estate"));
    }

    #[test]
    fn a_chapter_is_found_by_offset_when_there_are_no_page_markers() {
        let markdown = "# The Second Estate\n\nalpha beta gamma";
        let chunks = chunk(markdown, &small());
        assert_eq!(chunks[0].page, None);
        assert_eq!(chunks[0].chapter.as_deref(), Some("The Second Estate"));
    }

    #[test]
    fn a_chunk_below_the_minimum_is_dropped() {
        let budgets = Budgets {
            min_tokens: 100,
            ..small()
        };
        assert!(chunk("alpha beta", &budgets).is_empty());
    }

    #[test]
    fn nothing_exceeds_the_hard_maximum() {
        let markdown = "word ".repeat(500);
        let budgets = Budgets {
            target: 30,
            overlap: 5,
            min_tokens: 1,
            hard_max: 35,
        };
        for chunk in chunk(&markdown, &budgets) {
            assert!(chunk.tokens <= 35, "{} tokens", chunk.tokens);
        }
    }

    #[test]
    fn a_note_chunk_is_flagged_and_its_marker_is_stripped() {
        let markdown = format!(
            "{}12. Arendt, The Human Condition (Chicago, 1958), 44.\n\n\
             13. Bloch, Feudal Society, vol. 2, p. 17.",
            footnote::SENTINEL
        );
        let chunks = chunk(&markdown, &small());
        assert!(chunks[0].has_flag(flags::FOOTNOTE_CITATIONS));
        assert!(!chunks[0].text.contains(footnote::SENTINEL));
    }

    #[test]
    fn roles_are_smoothed_across_the_whole_document() {
        // Two bibliography-shaped pages with an ambiguous one between them.
        let entries = "Arendt, Hannah. The Human Condition. Chicago: University Press, 1958.\n\n\
                       Bloch, Marc. Feudal Society. London: Routledge, 1961.\n\n\
                       Cassirer, Ernst. The Myth of the State. New Haven: Yale Press, 1946.";
        let markdown = format!(
            "<!-- Page 1 -->\n\n{entries}\n\n<!-- Page 2 -->\n\nhttps://example.org/a\n\n\
             <!-- Page 3 -->\n\n{entries}"
        );
        let chunks = chunk(
            &markdown,
            &Budgets {
                target: 200,
                overlap: 0,
                min_tokens: 1,
                hard_max: 400,
            },
        );
        assert!(chunks.len() >= 3, "{chunks:#?}");
        assert!(
            chunks.iter().all(|chunk| chunk.role == Role::Bibliography),
            "{:?}",
            chunks.iter().map(|c| c.role).collect::<Vec<_>>()
        );
    }

    #[test]
    fn empty_input_produces_no_chunks() {
        assert!(chunk("", &Budgets::default()).is_empty());
        assert!(chunk("   \n\n  ", &Budgets::default()).is_empty());
    }
}
