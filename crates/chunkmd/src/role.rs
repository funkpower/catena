//! Telling body text from the apparatus around it.
//!
//! Retrieval wants body text. An index, a table of contents and a
//! bibliography all match a query beautifully and answer nothing — they are
//! lists of the words a reader is searching for. Marking them lets a search
//! exclude them while keeping them addressable for the consumers that do want
//! them: citation extraction reads bibliographies on purpose.
//!
//! Two signals, in order. The chapter title, when the document's headings
//! said what the section is; then the shape of the content, for back-matter
//! buried inside a body chapter — common in EPUBs whose bibliography heading
//! is not at the level the chapter map used.

use std::sync::LazyLock;

use corpus_core::Role;
use regex::Regex;

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

/// Chapter titles that give the section away, tried in this order.
static BY_TITLE: LazyLock<Vec<(Role, Regex)>> = LazyLock::new(|| {
    vec![
        (
            Role::Bibliography,
            pattern(
                r"(?i)(bibliograph|works\s+cited|literature\s+cited|further\s+reading|^\s*references\s*$|^\s*(end)?notes\s*$)",
            ),
        ),
        (Role::Index, pattern(r"(?i)^\s*index(\s+of\s+\w+)?\s*$")),
        (Role::Toc, pattern(r"(?i)^\s*(table\s+of\s+)?contents\s*$")),
        (
            Role::Frontmatter,
            pattern(
                r"(?i)(copyright|title\s*page|colophon|^\s*dedication\s*$|^\s*epigraph\s*$|about\s+the\s+(author|publisher)|acknowledg)",
            ),
        ),
    ]
});

/// A standalone bibliography heading inside the chunk's own text —
/// "Bibliography", "## Works Cited", "Selected Bibliography".
static INTEXT_BIBLIO_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?im)^\s*#{0,6}\s*(select(?:ed)?\s+)?(bibliography|works\s+cited|references|sources)\s*$",
    )
});

/// One bibliography entry: "Lastname, First…", or the dash run that stands
/// for "the same author again".
static BIBLIO_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^(?:[A-Z][\w'’\-]+,\s+\S|[—–-]{2,3}\s*[.,])"));

/// The continuation of a wrapped entry: "New York: Springer, 2013."
static BIBLIO_CONTINUATION: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"\b(University|Press|Publish|Books|Verlag|Editions|& Co|Inc\.)\b"));

static YEAR: LazyLock<Regex> = LazyLock::new(|| pattern(r"\b(1[5-9]\d{2}|20\d{2})\b"));

/// An index line: a short lead, then a list of page numbers.
static INDEX_LINE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^.{1,70}?,\s+[ivxlc\d]{1,4}(?:[,–-]\s*[ivxlc\d]{1,4})*\s*$"));

/// A contents line: dot leaders to a page number, or a link to an internal
/// anchor, which is what a converted EPUB's table of contents becomes.
static TOC_LINE: LazyLock<Regex> = LazyLock::new(|| pattern(r"(\.{3,}\s*\d{1,4}\s*$|\]\(#)"));

/// A wrapped entry long enough that its publisher name is probably a
/// coincidence rather than a citation, in characters.
const CONTINUATION_MAX: usize = 200;

/// How many entry-shaped paragraphs make a bibliography, and what share of
/// the paragraphs they have to be.
const BIBLIO_MIN_ENTRIES: usize = 3;
const BIBLIO_SHARE: f64 = 0.55;

/// The same, for an in-text bibliography heading, which is evidence on its
/// own and so needs less corroboration.
const BIBLIO_MIN_ENTRIES_WITH_HEADING: usize = 2;

/// How many list-shaped lines make an index or a table of contents.
const LIST_MIN_LINES: usize = 5;
const LIST_SHARE: f64 = 0.5;

/// Classify one chunk.
pub fn classify(text: &str, chapter: Option<&str>) -> Role {
    if let Some(chapter) = chapter.filter(|title| !title.is_empty()) {
        for (role, pattern) in BY_TITLE.iter() {
            if pattern.is_match(chapter) {
                return *role;
            }
        }
    }

    // Content-shape fallbacks. Cheap line scans; a chunk is a thousand
    // tokens at the outside.
    let lines: Vec<&str> = text
        .split('\n')
        .map(trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return Role::Body;
    }
    let paragraphs = textkit::paragraphs(text);

    let entryish = paragraphs
        .iter()
        .filter(|paragraph| looks_like_entry(paragraph))
        .count();
    let enough_entries =
        entryish >= BIBLIO_MIN_ENTRIES && entryish as f64 >= BIBLIO_SHARE * paragraphs.len() as f64;
    let headed =
        INTEXT_BIBLIO_HEADING.is_match(text) && entryish >= BIBLIO_MIN_ENTRIES_WITH_HEADING;
    if enough_entries || headed {
        return Role::Bibliography;
    }

    if dominates(
        lines
            .iter()
            .filter(|line| INDEX_LINE.is_match(line))
            .count(),
        lines.len(),
    ) {
        return Role::Index;
    }

    if dominates(
        lines.iter().filter(|line| TOC_LINE.is_match(line)).count(),
        lines.len(),
    ) {
        return Role::Toc;
    }

    Role::Body
}

fn looks_like_entry(paragraph: &str) -> bool {
    let year = YEAR.is_match(paragraph);
    let publisher = BIBLIO_CONTINUATION.is_match(paragraph);
    if BIBLIO_ENTRY.is_match(paragraph) && (year || publisher) {
        return true;
    }
    publisher && year && paragraph.chars().count() < CONTINUATION_MAX
}

fn dominates(matching: usize, total: usize) -> bool {
    matching >= LIST_MIN_LINES && matching as f64 >= LIST_SHARE * total as f64
}

/// Fix classification stragglers.
///
/// A run of at most two body chunks sandwiched between chunks of the same
/// non-body role inherits that role. This catches the boundary chunks of a
/// bibliography or an index, whose own content shape is ambiguous because
/// the entries there are URL-heavy, or split across the seam.
pub fn smooth(roles: &mut [Role]) {
    const MAX_RUN: usize = 2;

    let mut at = 0;
    while at < roles.len() {
        if roles[at] != Role::Body {
            at += 1;
            continue;
        }
        let mut end = at;
        while end < roles.len() && roles[end] == Role::Body {
            end += 1;
        }
        let before = at.checked_sub(1).map(|before| roles[before]);
        let after = roles.get(end).copied();
        if let Some(role) = before
            && end - at <= MAX_RUN
            && before == after
            && role != Role::Body
        {
            roles[at..end].fill(role);
        }
        at = end;
    }
}

fn trim(text: &str) -> &str {
    text.trim_matches(textkit::is_space)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chapter_title_decides_when_it_can() {
        for (title, role) in [
            ("Selected Bibliography", Role::Bibliography),
            ("Works Cited", Role::Bibliography),
            ("References", Role::Bibliography),
            ("Notes", Role::Bibliography),
            ("Endnotes", Role::Bibliography),
            ("Index", Role::Index),
            ("Index of Names", Role::Index),
            ("Table of Contents", Role::Toc),
            ("Contents", Role::Toc),
            ("Copyright", Role::Frontmatter),
            ("Acknowledgements", Role::Frontmatter),
            ("About the Author", Role::Frontmatter),
            ("III. The Second Estate", Role::Body),
        ] {
            assert_eq!(
                classify("ordinary text", Some(title)),
                role,
                "for {title:?}"
            );
        }
    }

    #[test]
    fn a_title_that_merely_mentions_references_in_a_sentence_is_not_a_bibliography() {
        assert_eq!(
            classify("text", Some("What the references tell us")),
            Role::Body
        );
    }

    #[test]
    fn entry_shaped_paragraphs_make_a_bibliography_without_a_title() {
        let text = "Arendt, Hannah. The Human Condition. Chicago: University Press, 1958.\n\n\
                    Bloch, Marc. Feudal Society. London: Routledge, 1961.\n\n\
                    Cassirer, Ernst. The Myth of the State. New Haven: Yale Press, 1946.";
        assert_eq!(classify(text, None), Role::Bibliography);
    }

    #[test]
    fn an_in_text_heading_lowers_the_bar_to_two_entries() {
        let text = "## Works Cited\n\n\
                    Arendt, Hannah. The Human Condition. Chicago: University Press, 1958.\n\n\
                    Bloch, Marc. Feudal Society. London: Routledge, 1961.";
        assert_eq!(classify(text, None), Role::Bibliography);
    }

    #[test]
    fn page_number_lists_make_an_index() {
        let text = "agriculture, 12, 34, 102\n\
                    bishops, 7\n\
                    charters, 88, 91\n\
                    demesne, 14-16\n\
                    estates, 3, 44\n\
                    fealty, 201";
        assert_eq!(classify(text, None), Role::Index);
    }

    #[test]
    fn dot_leaders_make_a_table_of_contents() {
        let text = "One. The Setting......1\n\
                    Two. The Quarrel......22\n\
                    Three. The Break......45\n\
                    Four. The Peace......70\n\
                    Five. After......99";
        assert_eq!(classify(text, None), Role::Toc);
    }

    #[test]
    fn ordinary_prose_is_body() {
        let text = "The second estate held its land in exchange for service, and \
                    the service was military. That much is agreed; what follows is not.";
        assert_eq!(classify(text, None), Role::Body);
    }

    #[test]
    fn empty_text_is_body() {
        assert_eq!(classify("   \n  ", None), Role::Body);
    }

    #[test]
    fn a_short_body_run_inside_a_bibliography_is_absorbed() {
        let mut roles = vec![
            Role::Bibliography,
            Role::Body,
            Role::Body,
            Role::Bibliography,
        ];
        smooth(&mut roles);
        assert_eq!(roles, [Role::Bibliography; 4]);
    }

    #[test]
    fn a_long_body_run_is_left_alone() {
        let mut roles = vec![
            Role::Bibliography,
            Role::Body,
            Role::Body,
            Role::Body,
            Role::Bibliography,
        ];
        let before = roles.clone();
        smooth(&mut roles);
        assert_eq!(roles, before);
    }

    #[test]
    fn a_run_between_different_roles_is_left_alone() {
        let mut roles = vec![Role::Bibliography, Role::Body, Role::Index];
        let before = roles.clone();
        smooth(&mut roles);
        assert_eq!(roles, before);
    }

    #[test]
    fn body_at_either_end_is_left_alone() {
        let mut roles = vec![Role::Body, Role::Index, Role::Body];
        let before = roles.clone();
        smooth(&mut roles);
        assert_eq!(roles, before);
    }
}
