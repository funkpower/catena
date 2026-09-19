//! Turning whatever a converter used for page boundaries into one marker.

use std::sync::LazyLock;

use regex::Regex;

/// The separator a paginating PDF converter writes between pages, like
/// `{0}------------------------------`. Its numbering is zero-based.
static SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{(\d+)\}-{20,}[\s\x{1c}-\x{1f}]*\n").expect("a valid pattern"));

/// A form feed, which is what a plain text extractor leaves between pages.
const FORM_FEED: char = '\u{c}';

/// Rewrite page boundaries as `<!-- Page N -->` markers.
///
/// Separators first, then form feeds, and text with neither is returned
/// unchanged — a cleanup pass has to be safe to run twice, and inventing a
/// page marker for a document that has none would not be.
pub fn normalize(text: &str) -> String {
    if SEPARATOR.is_match(text) {
        return from_separators(text);
    }
    if text.contains(FORM_FEED) {
        return from_form_feeds(text);
    }
    text.to_string()
}

fn from_separators(text: &str) -> String {
    let mut pages: Vec<String> = Vec::new();
    let mut at = 0;
    let mut number = 1;

    for found in SEPARATOR.captures_iter(text) {
        let whole = found.get(0).expect("group 0");
        let before = trim(&text[at..whole.start()]);
        if !before.is_empty() {
            pages.push(format!("<!-- Page {number} -->\n\n{before}"));
        }
        // The separator carries the number of the page it *ends*, counted
        // from zero, so the page that follows it is that number plus one.
        number = textkit::decimal(&found[1]).saturating_add(1);
        at = whole.end();
    }

    let last = trim(&text[at..]);
    if !last.is_empty() {
        pages.push(format!("<!-- Page {number} -->\n\n{last}"));
    }
    pages.join("\n\n")
}

fn from_form_feeds(text: &str) -> String {
    text.split(FORM_FEED)
        .map(trim)
        .enumerate()
        .filter(|(_, page)| !page.is_empty())
        .map(|(at, page)| format!("<!-- Page {} -->\n\n{page}", at + 1))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn trim(text: &str) -> &str {
    text.trim_matches(textkit::is_space)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_become_markers_numbered_from_one() {
        let text = "first page\n{0}------------------------------\nsecond page\n\
                    {1}------------------------------\nthird page";
        assert_eq!(
            normalize(text),
            "<!-- Page 1 -->\n\nfirst page\n\n\
             <!-- Page 1 -->\n\nsecond page\n\n\
             <!-- Page 2 -->\n\nthird page"
        );
    }

    #[test]
    fn form_feeds_become_markers() {
        let text = "one\u{c}two\u{c}three";
        assert_eq!(
            normalize(text),
            "<!-- Page 1 -->\n\none\n\n<!-- Page 2 -->\n\ntwo\n\n<!-- Page 3 -->\n\nthree"
        );
    }

    #[test]
    fn an_empty_page_is_dropped_and_the_numbering_carries_on() {
        assert_eq!(
            normalize("one\u{c}\u{c}three"),
            "<!-- Page 1 -->\n\none\n\n<!-- Page 3 -->\n\nthree"
        );
    }

    #[test]
    fn text_with_no_page_boundaries_is_left_exactly_as_it_was() {
        let text = "# A Heading\n\nSome prose, with no pagination at all.\n";
        assert_eq!(normalize(text), text);
    }

    #[test]
    fn a_short_dash_run_is_not_a_separator() {
        let text = "before\n{0}-----\nafter";
        assert_eq!(normalize(text), text);
    }
}
