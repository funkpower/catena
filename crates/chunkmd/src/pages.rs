//! Finding where the pages and the chapters are.
//!
//! Converted markdown carries page boundaries as `<!-- Page 41 -->` comments
//! and chapter boundaries as headings. Both are needed on every chunk: a
//! passage a reader cannot locate in the book is a passage they cannot check.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use textkit::pattern::SPACE;

static PAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"<!--{SPACE}*Page{SPACE}+(\d+){SPACE}*-->")).expect("a valid pattern")
});

static HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?m)^(#{{1,3}}){SPACE}+(.+)$")).expect("a valid pattern")
});

/// One page's worth of text, and where it started in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<'a> {
    /// The page number from the marker, absent for text that came before the
    /// first marker.
    pub number: Option<u32>,
    /// The page's text, trimmed.
    pub text: &'a str,
    /// Byte offset in the source markdown where `text` begins, used to find
    /// the chapter a passage sits under. Exact rather than approximate: it is
    /// added to a passage's own offset within the page, and a heading missed
    /// by a few bytes is the wrong chapter for everything after it.
    pub offset: usize,
}

/// A heading and the span of the document it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    /// The heading text, trimmed.
    pub title: String,
    /// Page the heading falls on, when the document has page markers.
    pub start_page: Option<u32>,
    /// Page the next heading falls on, or the last page.
    pub end_page: Option<u32>,
    /// Byte offset of the heading in the source markdown.
    pub start_offset: usize,
}

/// Split markdown on its page markers.
///
/// Text *before* the first marker is still the book. Starting the walk at the
/// first marker silently deleted it — which cost one EPUB 362k of its 364k
/// characters, because it carried exactly one stray anchor near the very end.
/// Nothing surfaced the loss: the book stayed ingested, with a single chunk.
/// So the prologue is emitted with no page number, since we genuinely do not
/// know its page, and the offset-based chapter lookup still tags it.
/// Where `part` begins inside `whole`, which it must be a slice of.
fn offset_of(whole: &str, part: &str) -> usize {
    (part.as_ptr() as usize).saturating_sub(whole.as_ptr() as usize)
}

/// Cut `markdown` at its page markers.
pub fn split_by_page(markdown: &str) -> Vec<Page<'_>> {
    let markers: Vec<_> = PAGE.find_iter(markdown).collect();
    if markers.is_empty() {
        return vec![Page {
            number: None,
            text: markdown,
            offset: 0,
        }];
    }

    let mut pages = Vec::new();
    let head = trim(&markdown[..markers[0].start()]);
    if !head.is_empty() {
        pages.push(Page {
            number: None,
            text: head,
            offset: offset_of(markdown, head),
        });
    }
    for (at, marker) in markers.iter().enumerate() {
        let start = marker.end();
        let end = markers
            .get(at + 1)
            .map_or(markdown.len(), |next| next.start());
        let text = trim(&markdown[start..end]);
        if !text.is_empty() {
            pages.push(Page {
                number: Some(textkit::decimal(&markdown[marker.start()..marker.end()])),
                text,
                offset: offset_of(markdown, text),
            });
        }
    }
    pages
}

/// Extract the headings that act as chapters, with their page and byte spans.
///
/// A document that has H1s is chaptered by its H1s; one that only has H2s is
/// chaptered by those. Mixing levels would make a chapter list where a
/// section and the chapter containing it sit side by side.
pub fn chapters(markdown: &str) -> Vec<Chapter> {
    let mut headings: Vec<(usize, String, usize)> = Vec::new();
    for found in HEADING.captures_iter(markdown) {
        let title = trim(found.get(2).expect("group 2").as_str());
        if title.is_empty() {
            continue;
        }
        headings.push((
            found.get(1).expect("group 1").len(),
            title.to_string(),
            found.get(0).expect("group 0").start(),
        ));
    }
    if headings.is_empty() {
        return Vec::new();
    }

    // The shallowest level that divides the document, which is not always the
    // shallowest level in it. A converted EPUB carries the book's title as a
    // lone `#` above sixty-two `##` chapters, and taking the lone one makes
    // the whole book a single chapter — so a level used once is a title and a
    // level used twice is a chapter map.
    let mut counts: BTreeMap<usize, usize> = BTreeMap::new();
    for (level, _, _) in &headings {
        *counts.entry(*level).or_default() += 1;
    }
    let dividing = counts
        .iter()
        .find(|(_, used)| **used > 1)
        .map(|(level, _)| *level)
        .or_else(|| counts.keys().next().copied())
        .expect("a heading");
    headings.retain(|(level, _, _)| *level == dividing);

    let page_at = PageIndex::new(markdown);
    let mut chapters = Vec::with_capacity(headings.len());
    for (at, (_, title, offset)) in headings.iter().enumerate() {
        let end = headings
            .get(at + 1)
            .map_or(markdown.len(), |(_, _, next)| *next);
        chapters.push(Chapter {
            title: title.clone(),
            start_page: page_at.at(*offset),
            end_page: page_at.at(end),
            start_offset: *offset,
        });
    }
    chapters
}

/// Which page a byte offset falls on.
pub struct PageIndex(Vec<(usize, u32)>);

impl PageIndex {
    /// Index the page markers in `markdown`.
    pub fn new(markdown: &str) -> Self {
        Self(
            PAGE.find_iter(markdown)
                .map(|marker| (marker.start(), textkit::decimal(marker.as_str())))
                .collect(),
        )
    }

    /// The last page marker at or before `offset`.
    pub fn at(&self, offset: usize) -> Option<u32> {
        let found = self.0.partition_point(|(start, _)| *start <= offset);
        found.checked_sub(1).map(|at| self.0[at].1)
    }
}

/// Which chapter a byte offset falls in.
pub struct ChapterIndex<'a>(&'a [Chapter]);

impl<'a> ChapterIndex<'a> {
    /// Index a chapter list, which must be in document order.
    pub fn new(chapters: &'a [Chapter]) -> Self {
        Self(chapters)
    }

    /// The last chapter starting at or before `offset`.
    pub fn at(&self, offset: usize) -> Option<&'a str> {
        let found = self.0.partition_point(|c| c.start_offset <= offset);
        found.checked_sub(1).map(|at| self.0[at].title.as_str())
    }
}

/// The chapter covering a page, if any.
///
/// Where chapters overlap — which happens when two headings land on one page
/// — the later one wins, because that is the one a reader scrolling down
/// would be under.
///
/// Scanned rather than indexed into a map of every page. A document has tens
/// of chapters and a scan of them costs nothing, where a map costs one entry
/// per page in every chapter's span: a single mistyped marker reading
/// `<!-- Page 20231115 -->` would ask for a gigabyte of them.
pub fn chapter_of_page(chapters: &[Chapter], page: u32) -> Option<&str> {
    // The last chapter that covers the page, not the first: chapters can
    // nest, and the innermost one is the one a reader would name.
    chapters
        .iter()
        .rfind(|chapter| {
            let Some(start) = chapter.start_page else {
                return false;
            };
            // A zero end page means the same thing as no end page: this
            // chapter is one page long as far as anyone can tell.
            let end = match chapter.end_page {
                Some(0) | None => start,
                Some(end) => end,
            };
            (start..=end.max(start)).contains(&page) && end >= start
        })
        .map(|chapter| chapter.title.as_str())
}

fn trim(text: &str) -> &str {
    text.trim_matches(textkit::is_space)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_with_no_markers_is_one_unnumbered_page() {
        let pages = split_by_page("just text");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].number, None);
        assert_eq!(pages[0].text, "just text");
    }

    #[test]
    fn text_before_the_first_marker_is_kept() {
        let pages = split_by_page("prologue\n\n<!-- Page 19 -->\n\nbody");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].number, None);
        assert_eq!(pages[0].text, "prologue");
        assert_eq!(pages[1].number, Some(19));
        assert_eq!(pages[1].text, "body");
    }

    #[test]
    fn an_empty_page_is_dropped() {
        let pages = split_by_page("<!-- Page 1 -->\n\n<!-- Page 2 -->\n\nbody");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].number, Some(2));
    }

    #[test]
    fn markers_may_be_spelled_loosely() {
        let pages = split_by_page("<!--Page   7-->\n\nbody");
        assert_eq!(pages[0].number, Some(7));
    }

    #[test]
    fn a_lone_shallow_heading_is_a_title_rather_than_a_chapter_map() {
        // What a converted EPUB looks like: the book's title as the only
        // `#`, and every actual chapter a `##` under it. Taking the lone
        // one makes the whole book a single chapter.
        let markdown = "# The Whole Book\n\n## Chapter One\n\ntext\n\n## Chapter Two\n\ntext";
        let found = chapters(markdown);
        assert_eq!(
            found
                .iter()
                .map(|one| one.title.as_str())
                .collect::<Vec<_>>(),
            ["Chapter One", "Chapter Two"]
        );
    }

    #[test]
    fn chapters_come_from_the_shallowest_heading_level_present() {
        let markdown = "## Alpha\n\ntext\n\n### Sub\n\ntext\n\n## Beta\n\ntext";
        let found = chapters(markdown);
        assert_eq!(
            found.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(),
            ["Alpha", "Beta"]
        );
    }

    #[test]
    fn a_heading_with_nothing_after_it_is_not_a_chapter() {
        assert!(chapters("# \n\n").is_empty());
    }

    #[test]
    fn a_hash_on_its_own_line_takes_the_next_line_as_its_title() {
        // The whitespace between the hashes and the title may include the
        // newline, so a bare `#` adopts what follows it. Faithful to the
        // Python, and the case does not arise in converted markdown.
        let found = chapters("#    \n\ntext");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "text");
    }

    #[test]
    fn chapters_carry_the_pages_they_span() {
        let markdown = "<!-- Page 1 -->\n\n# One\n\ntext\n\n<!-- Page 4 -->\n\n# Two\n\ntext";
        let found = chapters(markdown);
        assert_eq!(found[0].start_page, Some(1));
        assert_eq!(found[0].end_page, Some(4));
        assert_eq!(found[1].start_page, Some(4));
        assert_eq!(chapter_of_page(&found, 1), Some("One"));
        assert_eq!(chapter_of_page(&found, 3), Some("One"));
        assert_eq!(chapter_of_page(&found, 4), Some("Two"));
        assert_eq!(chapter_of_page(&found, 9), None);
    }

    #[test]
    fn an_offset_before_the_first_heading_has_no_chapter() {
        let found = chapters("front\n\n# One\n\ntext");
        let index = ChapterIndex::new(&found);
        assert_eq!(index.at(0), None);
        assert_eq!(index.at(10), Some("One"));
    }
}
