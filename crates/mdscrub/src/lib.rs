//! Cleans converted markdown.
//!
//! Conversion from a PDF or an EPUB leaves markdown that is readable but not
//! clean: styling tags that survived, control characters a text column will
//! reject, headings wrapped in links to their own anchors, and page
//! boundaries spelled three different ways. None of it means anything
//! downstream, and all of it ends up in an index.
//!
//! The order of the passes matters. Page boundaries are normalised first,
//! because one of the things that marks a page boundary — a form feed — is
//! also a control character that the later pass deletes.

mod entities;
pub mod heading;
pub mod html;
pub mod pages;
pub mod unescape;

/// Clean a converted markdown document.
///
/// Safe to run twice: the second run changes nothing.
pub fn scrub(markdown: &str) -> String {
    let text = heading::clean_all(markdown);
    let text = heading::anchors_to_markers(&text);
    let text = html::strip_tags(&text);
    let text = html::scrub_control(&text);
    let text = html::collapse_blank_runs(&text);
    format!("{}\n", text.trim_matches(textkit::is_space))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_converted_heading_comes_out_readable() {
        let markdown = "# [<span id=\"page12\"></span>I. The Setting](#ch1) {.unnumbered}";
        assert_eq!(scrub(markdown), "# I. The Setting\n\n<!-- Page 12 -->\n");
    }

    #[test]
    fn running_it_twice_changes_nothing() {
        let markdown = "<!-- Page 3 -->\n\n# The <em>Quarrel</em>\n\n\n\nA <b>mill</b>\u{0} \
                        stood here.<br>It still does.\n\n<span id=\"page4\"></span>\n\nOn.";
        let once = scrub(markdown);
        assert_eq!(scrub(&once), once);
    }

    #[test]
    fn a_page_marker_survives_every_pass() {
        assert!(scrub("<!-- Page 41 -->\n\ntext").contains("<!-- Page 41 -->"));
    }

    #[test]
    fn the_result_always_ends_in_exactly_one_newline() {
        for markdown in ["", "text", "text\n\n\n", "  \n"] {
            let cleaned = scrub(markdown);
            assert!(cleaned.ends_with('\n'), "{markdown:?}");
            assert!(!cleaned.ends_with("\n\n"), "{markdown:?}");
        }
    }

    #[test]
    fn page_normalisation_is_not_part_of_cleaning() {
        // A form feed in ordinary markdown is a control character, not a page
        // boundary. It is one only to a program that knows it just read a PDF.
        assert_eq!(scrub("one\u{c}two"), "onetwo\n");
    }

    #[test]
    fn pagination_first_then_cleaning_is_still_idempotent() {
        let raw = "one\u{c}<span>two</span>";
        let once = scrub(&pages::normalize(raw));
        assert!(once.contains("<!-- Page 2 -->"), "{once:?}");
        assert_eq!(scrub(&pages::normalize(&once)), once);
    }

    #[test]
    fn nothing_manufactures_a_page_boundary_on_a_second_run() {
        // Stripping a tag can leave a separator flush against a newline. If
        // pagination ran after that, a second pass would find a boundary the
        // first pass did not.
        let raw = "{2}------------------------------<span></span>\nafter\n";
        let once = scrub(raw);
        assert_eq!(scrub(&once), once, "{once:?}");
    }
}
