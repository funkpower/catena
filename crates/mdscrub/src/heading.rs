//! Making a heading line readable again.
//!
//! A converted heading arrives carrying whatever the source used to style it:
//! link wrappers around the whole title, attribute blocks, anchor spans, bold
//! markers around a title that was set in capitals. The heading text is what
//! every chunk in the chapter will be labelled with, so it is worth cleaning
//! properly rather than passing through.

use std::sync::LazyLock;

use regex::Regex;
use textkit::pattern::SPACE;

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

/// `[text](url)` — a heading wrapped in a link to its own anchor.
static LINK: LazyLock<Regex> = LazyLock::new(|| pattern(r"\[([^\]]*)\]\([^)]*\)"));

/// `[text]{.cls}` — pandoc's bracketed inline span. The text may be empty:
/// `[]{#chapter-3}` is how an anchor with nothing in it arrives, and a
/// heading that keeps one is a chapter title with an element id in it.
static BRACKETED: LazyLock<Regex> = LazyLock::new(|| pattern(r"\[([^\]]*)\]\{[^}]*\}"));

/// A trailing `{.attr}` block.
static ATTRIBUTE_TAIL: LazyLock<Regex> =
    LazyLock::new(|| pattern(&format!(r"{SPACE}*\{{[^}}]*\}}{SPACE}*$")));

/// Any inline tag. Broader than the one used on body text, because a heading
/// has no code fences to protect.
static TAG: LazyLock<Regex> = LazyLock::new(|| pattern(r"<[^>]+>"));

static SPACE_RUN: LazyLock<Regex> = LazyLock::new(|| pattern(&format!(r"{SPACE}+")));

/// A heading line, and the text after its hashes.
static HEADING: LazyLock<Regex> = LazyLock::new(|| pattern(&format!(r"^(#{{1,6}}){SPACE}+(.*)$")));

/// A page anchor: `<span id="…page42">`, `<a id="page-42">`, and the rest of
/// the shapes an EPUB uses to say where a printed page began.
///
/// On the backtracking engine, as the Python is. A tag carrying an `id` that
/// is not a page — `<span id="filepos1024631">` is what a Kindle conversion
/// leaves behind, and one real book has forty-four of them — makes the two
/// `[^"']` runs and the two `[^>]` runs try every split between them before
/// giving up. On a megabyte of markdown that does not finish. The engine here
/// cannot backtrack, so no input can make it.
///
/// The word boundaries are this engine's `\b` rather than Python's, which
/// differ only around combining marks and connector punctuation — neither of
/// which occurs between `<span` and `id` in anything a converter emits.
pub static PAGE_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r#"(?i)<(?:span|a|div)\b[^>]*\bid[\s]*=[\s]*["'][^"']*?page[-_]?(\d+)[^"']*["'][^>]*>(?:</(?:span|a|div)>)?"#,
    )
});

/// Clean one heading's text.
pub fn clean(raw: &str) -> String {
    let text = LINK.replace_all(raw, "$1");
    let text = BRACKETED.replace_all(&text, "$1");
    let text = ATTRIBUTE_TAIL.replace_all(&text, "");
    let text = TAG.replace_all(&text, "");
    let text = SPACE_RUN.replace_all(&text, " ");
    let text = text.trim_matches(textkit::is_space);
    // Emphasis markers around a title that was set in capitals say nothing
    // once the title is a label on a search result.
    text.replace("**", "").replace("__", "")
}

/// Clean every heading in a document, moving any page anchor found in a
/// heading to a marker on its own line just after it.
///
/// After the heading rather than before, so that the heading keeps its offset
/// relative to the body text that follows, and the chapter title stays whole.
pub fn clean_all(markdown: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in markdown.split('\n') {
        let Some(found) = HEADING.captures(line) else {
            out.push(line.to_string());
            continue;
        };
        let hashes = &found[1];
        let raw = &found[2];
        let pages: Vec<u32> = PAGE_ANCHOR
            .captures_iter(raw)
            .map(|anchor| textkit::decimal(&anchor[1]))
            .collect();
        let cleaned = clean(&PAGE_ANCHOR.replace_all(raw, ""));
        if !cleaned.is_empty() {
            out.push(format!("{hashes} {cleaned}"));
        }
        for page in pages {
            out.push(String::new());
            out.push(format!("<!-- Page {page} -->"));
        }
    }
    out.join("\n")
}

/// Turn the page anchors left in the body into page markers.
///
/// The number is carried through exactly as the document wrote it, padding
/// and all: `page007` becomes page 007. A heading's anchor is read as a
/// number instead, because a heading's page is a fact about the document
/// rather than a piece of its text.
pub fn anchors_to_markers(markdown: &str) -> String {
    PAGE_ANCHOR
        .replace_all(markdown, |found: &regex::Captures| {
            format!("\n\n<!-- Page {} -->\n\n", &found[1])
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_whose_id_is_not_a_page_finishes() {
        // What a Kindle conversion leaves behind, forty-four times in one
        // real book. On a backtracking engine the two `[^"']` runs and the
        // two `[^>]` runs try every split between them before giving up, and
        // on a megabyte of it that never returns.
        let junk = "<span id=\"filepos1024631\">text</span>\n\n".repeat(400);
        let markdown = format!("# Heading\n\n{junk}<span id=\"page42\"></span>\n");
        let out = anchors_to_markers(&markdown);
        assert!(
            out.contains("<!-- Page 42 -->"),
            "the real anchor is still found"
        );
        assert!(out.contains("filepos1024631"), "the others are left alone");
    }

    #[test]
    fn a_link_wrapper_comes_off() {
        assert_eq!(clean("[I. The Setting](#ch1)"), "I. The Setting");
    }

    #[test]
    fn a_bracketed_span_comes_off() {
        assert_eq!(clean("[The Quarrel]{.chapter-title}"), "The Quarrel");
    }

    #[test]
    fn a_trailing_attribute_block_comes_off() {
        assert_eq!(clean("The Break {#ch3 .unnumbered}"), "The Break");
    }

    #[test]
    fn tags_come_off_and_whitespace_collapses() {
        assert_eq!(clean("<span id=\"x\">The   Break</span>  "), "The Break");
    }

    #[test]
    fn emphasis_markers_come_off() {
        assert_eq!(clean("**THE SETTLEMENT**"), "THE SETTLEMENT");
    }

    #[test]
    fn a_page_anchor_in_a_heading_becomes_a_marker_after_it() {
        let cleaned = clean_all("# <span id=\"page42\"></span>The Quarrel");
        assert_eq!(cleaned, "# The Quarrel\n\n<!-- Page 42 -->");
    }

    #[test]
    fn a_heading_that_was_only_an_anchor_leaves_only_the_marker() {
        let cleaned = clean_all("## <a id=\"page-7\"></a>");
        assert_eq!(cleaned, "\n<!-- Page 7 -->");
    }

    #[test]
    fn body_anchors_become_markers() {
        let cleaned = anchors_to_markers("text <span id=\"pg_page19\"></span> more");
        assert!(cleaned.contains("<!-- Page 19 -->"));
    }

    #[test]
    fn a_padded_page_number_in_the_body_keeps_its_padding() {
        let cleaned = anchors_to_markers("text <span id=\"page007\"></span> more");
        assert!(cleaned.contains("<!-- Page 007 -->"), "{cleaned}");
    }

    #[test]
    fn a_line_that_is_not_a_heading_is_untouched() {
        assert_eq!(
            clean_all("#no space is not a heading"),
            "#no space is not a heading"
        );
    }
}
