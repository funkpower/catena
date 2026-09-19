//! Removing the HTML that survives conversion to markdown.

use std::sync::LazyLock;

use regex::Regex;

/// An opening or closing inline tag.
///
/// Converted markdown is full of them: pandoc's commonmark output keeps an
/// EPUB's styling spans and anchors, and a PDF converter emits anchor spans
/// and superscript note references. None of it means anything once the page
/// anchors have been pulled out, and all of it pollutes an embedding, a
/// full-text index, and every result a reader sees.
///
/// Comments are deliberately not matched: `<!-- Page 41 -->` has to survive.
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"</?[A-Za-z][^>]*>").expect("valid"));

/// A tag that stands for a line break rather than for styling.
static BREAK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").expect("valid"));

/// The start of a fenced code block.
static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(```|~~~)").expect("valid"));

/// Control characters that must never reach a text column.
///
/// A database will reject `0x00` outright. The other C0 controls — tab,
/// newline and carriage return excepted — are storable and are extraction
/// garbage: bad scans and malformed PDFs emit them routinely.
static CONTROL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\x00-\x08\x0b\x0c\x0e-\x1f]").expect("valid"));

/// Three or more newlines, left behind wherever a tag-only line was removed.
static BLANK_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").expect("valid"));

/// Strip inline HTML, keeping the text inside it and the HTML comments.
///
/// Fenced code blocks are left alone, so a book that quotes HTML keeps its
/// examples. `<br>` becomes a line break rather than nothing, because
/// deleting it welds the words on either side into one — which is a spelling
/// mistake in the index that no reader can search for.
pub fn strip_tags(markdown: &str) -> String {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in markdown.split('\n') {
        if FENCE.is_match(line) {
            in_fence = !in_fence;
            out.push(line.to_string());
            continue;
        }
        if in_fence {
            out.push(line.to_string());
        } else {
            let line = BREAK.replace_all(line, "\n");
            out.push(TAG.replace_all(&line, "").into_owned());
        }
    }
    out.join("\n")
}

/// Remove the control characters a text column cannot hold.
pub fn scrub_control(text: &str) -> String {
    CONTROL.replace_all(text, "").into_owned()
}

/// Collapse runs of blank lines to a single blank line.
pub fn collapse_blank_runs(text: &str) -> String {
    BLANK_RUN.replace_all(text, "\n\n").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styling_tags_go_and_their_text_stays() {
        assert_eq!(
            strip_tags("<span class=\"c1\">The</span> second <em>estate</em>"),
            "The second estate"
        );
    }

    #[test]
    fn html_comments_survive() {
        assert_eq!(strip_tags("<!-- Page 41 -->"), "<!-- Page 41 -->");
    }

    #[test]
    fn a_break_tag_does_not_weld_the_words_either_side_of_it() {
        assert_eq!(strip_tags("first<br>second"), "first\nsecond");
        assert_eq!(strip_tags("first<br />second"), "first\nsecond");
        assert_eq!(strip_tags("first<BR/>second"), "first\nsecond");
    }

    #[test]
    fn fenced_code_keeps_its_tags() {
        let markdown = "before <b>x</b>\n\n```html\n<span>kept</span>\n```\n\nafter <b>y</b>";
        let cleaned = strip_tags(markdown);
        assert!(cleaned.contains("<span>kept</span>"));
        assert!(cleaned.contains("before x"));
        assert!(cleaned.contains("after y"));
    }

    #[test]
    fn a_tag_split_across_two_lines_is_left_alone() {
        // Matching per line is what keeps a stray `<` in prose from eating
        // the rest of the document.
        let markdown = "a < b and\nc > d";
        assert_eq!(strip_tags(markdown), markdown);
    }

    #[test]
    fn control_characters_go_but_tabs_and_newlines_stay() {
        assert_eq!(scrub_control("a\u{0}b\u{1}c"), "abc");
        assert_eq!(scrub_control("a\tb\nc\r\n"), "a\tb\nc\r\n");
    }

    #[test]
    fn blank_line_runs_collapse_to_one() {
        assert_eq!(collapse_blank_runs("a\n\n\n\n\nb"), "a\n\nb");
        assert_eq!(collapse_blank_runs("a\n\nb"), "a\n\nb");
    }
}
