//! Removing the HTML that survives conversion to markdown.

use std::sync::LazyLock;

use regex::Regex;

/// Python's `\s`, which is the Unicode whitespace property plus the four
/// ASCII information separators. Spelled out wherever a ported pattern uses
/// `\s`, because the difference decides where a page boundary is.
const SPACE: &str = r"[\s\x{1c}-\x{1f}]";

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

/// An opening or closing inline tag, and its name.
///
/// Converted markdown is full of them: pandoc's commonmark output keeps an
/// EPUB's styling spans and anchors, and a PDF converter emits anchor spans
/// and superscript note references. None of it means anything once the page
/// anchors have been pulled out, and all of it pollutes an embedding, a
/// full-text index, and every result a reader sees.
///
/// Comments are deliberately not matched: `<!-- Page 41 -->` has to survive.
static TAG: LazyLock<Regex> = LazyLock::new(|| pattern(r"</?([A-Za-z][A-Za-z0-9]*)[^>]*>"));

/// A markdown image carrying its payload inline.
///
/// A generated EPUB cover arrives this way — a few hundred characters of
/// base64 in a wrapper — and because the cover is the spine's first document
/// it lands in the first chunk of the book, which is the chunk a "what is
/// this book about" query matches. It holds no text, so it can only dilute
/// the chunk it sits in.
static DATA_URI_IMAGE: LazyLock<Regex> =
    LazyLock::new(|| pattern(&format!(r"!\[[^\]]*\]\({SPACE}*data:[^)]*\)")));

/// Control characters that must never reach a text column.
///
/// A database will reject `0x00` outright. The other C0 controls — tab,
/// newline and carriage return excepted — are storable and are extraction
/// garbage: bad scans and malformed PDFs emit them routinely.
static CONTROL: LazyLock<Regex> = LazyLock::new(|| pattern(r"[\x00-\x08\x0b\x0c\x0e-\x1f]"));

/// Three or more newlines, left behind wherever a tag-only line was removed.
static BLANK_RUN: LazyLock<Regex> = LazyLock::new(|| pattern(r"\n{3,}"));

/// Tags that stand for a line or block boundary rather than a span of
/// styling.
///
/// Deleting one outright welds the words either side of it together: a table
/// of contents reading `APPENDICES<br>OF<br>LIST<br>xii` comes out as
/// `APPENDICESOFLISTxii`, which is a word no reader can search for and no
/// tokeniser can split. These become a space instead. Genuinely inline tags
/// still go to nothing, because `claim<sup>16</sup>` has to stay `claim16`.
const BREAK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "br",
    "caption",
    "dd",
    "div",
    "dl",
    "dt",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
];

/// Strip inline HTML, keeping the text inside it and the HTML comments.
///
/// Character references are decoded first: a tag that was escaped twice
/// arrives as `&lt;sup>107` and no tag pattern can see it, so the markup
/// would otherwise survive into the text. Fenced code blocks are left alone,
/// so a book that quotes HTML keeps its examples.
pub fn strip_tags(markdown: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in markdown.split('\n') {
        if opens_a_fence(line) {
            in_fence = !in_fence;
            out.push(line.to_string());
            continue;
        }
        if in_fence {
            out.push(line.to_string());
            continue;
        }
        let text = crate::unescape::unescape(line);
        let text = DATA_URI_IMAGE.replace_all(&text, "");
        let text = TAG.replace_all(
            &text,
            |found: &regex::Captures| {
                if is_break_tag(&found[1]) { " " } else { "" }
            },
        );
        // A line that held nothing but tags becomes empty rather than a lone
        // space, so the blank-line collapse below still sees it as blank.
        if text.trim_matches(textkit::is_space).is_empty() {
            out.push(String::new());
        } else {
            out.push(text.into_owned());
        }
    }
    out.join("\n")
}

/// Whether a line opens or closes a fenced code block.
///
/// A run of at least three backticks or tildes whose info string does not
/// itself contain the fence character. The looser reading — any three of
/// them — treats a decorative rule as a fence: `~~~~~~~~~END~~~~~~~~~` in a
/// scanned book switched cleaning off for the rest of the document and left
/// the markup behind.
fn opens_a_fence(line: &str) -> bool {
    let line = line.trim_start_matches(textkit::is_space);
    ['`', '~'].into_iter().any(|fence| {
        let rest = line.trim_start_matches(fence);
        line.len() - rest.len() >= 3 && !rest.contains(fence)
    })
}

fn is_break_tag(name: &str) -> bool {
    let name = name.to_lowercase();
    BREAK_TAGS.binary_search(&name.as_str()).is_ok()
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
    fn the_break_tag_list_is_sorted_so_it_can_be_searched() {
        assert!(BREAK_TAGS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn styling_tags_go_and_their_text_stays() {
        assert_eq!(
            strip_tags("<span class=\"c1\">The</span> second <em>estate</em>"),
            "The second estate"
        );
    }

    #[test]
    fn an_inline_tag_leaves_nothing_behind() {
        assert_eq!(strip_tags("claim<sup>16</sup>"), "claim16");
    }

    #[test]
    fn a_block_tag_leaves_a_space_so_words_do_not_weld() {
        assert_eq!(strip_tags("APPENDICES<br>OF<br>LIST"), "APPENDICES OF LIST");
        assert_eq!(strip_tags("<p>one</p><p>two</p>"), " one  two ");
        assert_eq!(strip_tags("a<BR/>b"), "a b");
    }

    #[test]
    fn html_comments_survive() {
        assert_eq!(strip_tags("<!-- Page 41 -->"), "<!-- Page 41 -->");
    }

    #[test]
    fn a_doubly_escaped_tag_is_decoded_and_then_removed() {
        assert_eq!(strip_tags("&lt;sup>107Ibid."), "107Ibid.");
    }

    #[test]
    fn an_inline_image_payload_goes() {
        let line = "![cover](data:image/svg+xml;base64,PHN2ZyB4bWxucz0i) and text";
        assert_eq!(strip_tags(line), " and text");
    }

    #[test]
    fn fenced_code_keeps_its_tags_and_its_entities() {
        let markdown = "before <b>x</b>\n\n```html\n<span>kept</span>\n&amp; kept\n```\n\nafter";
        let cleaned = strip_tags(markdown);
        assert!(cleaned.contains("<span>kept</span>"));
        assert!(cleaned.contains("&amp; kept"));
        assert!(cleaned.contains("before x"));
    }

    #[test]
    fn a_decorative_rule_is_not_a_fence() {
        let markdown = "~~~~~~~~~END~~~~~~~~~\n<span class=\"calibre_10\">after</span>";
        assert_eq!(strip_tags(markdown), "~~~~~~~~~END~~~~~~~~~\nafter");
    }

    #[test]
    fn a_real_fence_with_an_info_string_still_opens() {
        assert!(opens_a_fence("```html"));
        assert!(opens_a_fence("   ~~~"));
        assert!(!opens_a_fence("``"));
        assert!(!opens_a_fence("text ```"));
    }

    #[test]
    fn a_line_of_nothing_but_tags_becomes_blank() {
        assert_eq!(strip_tags("a\n<span></span>\nb"), "a\n\nb");
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
