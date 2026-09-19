//! Making a heading line readable again.
//!
//! A converted heading arrives carrying whatever the source used to style it:
//! link wrappers around the whole title, attribute blocks, anchor spans, bold
//! markers around a title that was set in capitals. The heading text is what
//! every chunk in the chapter will be labelled with, so it is worth cleaning
//! properly rather than passing through.

use std::sync::LazyLock;

use regex::Regex;

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

/// `[text](url)` — a heading wrapped in a link to its own anchor.
static LINK: LazyLock<Regex> = LazyLock::new(|| pattern(r"\[([^\]]*)\]\([^)]*\)"));

/// `[text]{.cls}` — pandoc's bracketed inline span.
static BRACKETED: LazyLock<Regex> = LazyLock::new(|| pattern(r"\[([^\]]+)\]\{[^}]*\}"));

/// A trailing `{.attr}` block.
static ATTRIBUTE_TAIL: LazyLock<Regex> = LazyLock::new(|| pattern(r"\s*\{[^}]*\}\s*$"));

/// Any inline tag. Broader than the one used on body text, because a heading
/// has no code fences to protect.
static TAG: LazyLock<Regex> = LazyLock::new(|| pattern(r"<[^>]+>"));

static SPACE_RUN: LazyLock<Regex> = LazyLock::new(|| pattern(r"\s+"));

/// A heading line, and the text after its hashes.
static HEADING: LazyLock<Regex> = LazyLock::new(|| pattern(r"^(#{1,6})\s+(.*)$"));

/// A page anchor: `<span id="…page42">`, `<a id="page-42">`, and the rest of
/// the shapes an EPUB uses to say where a printed page began.
pub static PAGE_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r#"(?i)<(?:span|a|div)\b[^>]*\bid\s*=\s*["'][^"']*?page[-_]?(\d+)[^"']*["'][^>]*>(?:</(?:span|a|div)>)?"#,
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
pub fn anchors_to_markers(markdown: &str) -> String {
    PAGE_ANCHOR
        .replace_all(markdown, |found: &regex::Captures| {
            format!("\n\n<!-- Page {} -->\n\n", textkit::decimal(&found[1]))
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_line_that_is_not_a_heading_is_untouched() {
        assert_eq!(
            clean_all("#no space is not a heading"),
            "#no space is not a heading"
        );
    }
}
