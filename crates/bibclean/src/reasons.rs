//! What is wrong with a record, and how badly.

use std::sync::LazyLock;

use fancy_regex::Regex as Boundaried;
use regex::Regex;
use textkit::pattern::{SPACE, WORD_END, WORD_START};

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

fn bounded(source: &str) -> Boundaried {
    Boundaried::new(source).expect("a valid pattern")
}

/// A bare filename sitting in the title field.
static FILENAME_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(?i)\.(png|jpe?g|pdf|epub|mobi|azw3|azw|txt|wpd|docx?|rtf|tex|qxd|indd|pm\d){SPACE}*$"
    ))
});

/// A title that names the tool that made the file rather than the work.
///
/// Authoring and desktop-publishing programs stamp their scratch filename as
/// the PDF title; scanners and converters name themselves. The last of these
/// is a Turkish locale's "full page fax print" — the same driver, localised.
static JUNK_TITLE: LazyLock<Boundaried> = LazyLock::new(|| {
    bounded(&format!(
        r"(?i)microsoft word|z-?lib|libgen|\(z-lib|annas?-?archive|{WORD_START}ocr{WORD_END}|nota bene|pagemaker|quarkxpress|adobe indesign|acrobat distiller|word ?pro|wordperfect|{WORD_START}dvips{WORD_END}|{WORD_START}ltx{WORD_END}|{WORD_START}pdftex{WORD_END}|djvu document|untitled document|full page fax print|tam sayfa faks"
    ))
});

/// A path sitting in the title field. Distinct from a filename, which is a
/// bare name ending in an extension.
static PATH_TITLE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"(^[A-Za-z]:[\\/])|(\\\\[^\\]+\\)|(^/(?:home|users|mnt|tmp|var)/)"));

/// A title that is a placeholder rather than a name.
///
/// Anchored and whole-string: a real book called "The Document" or "Test"
/// must not be swept up, so these only match when the entire field is the
/// placeholder.
static PLACEHOLDER_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)^\W*(?:some[\s_-]*title|no[\s_-]*title|untitled|unnamed|title|document|documento|test|sample|default|placeholder|tbd|todo|draft|temp|new[\s_-]*document|book\d*|file\d*|scan\d*|doc\d*|n/?a|null|none|x{3,}|unknown)\W*$",
    )
});

static PLACEHOLDER_AUTHOR: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)^\W*(?:some[\s_-]*author|no[\s_-]*author|unnamed|author|default|test|sample|placeholder|tbd|todo|n/?a|null|none|x{3,}|administrator|user|owner|admin)\W*$",
    )
});

/// An author field that opens with a bare three or four digit number is a
/// publication year or a series index in the wrong column, never a person.
/// Anchored, so a real name containing a number is untouched.
static NUMERIC_AUTHOR: LazyLock<Regex> =
    LazyLock::new(|| pattern(&format!(r"^{SPACE}*\d{{3,4}}{SPACE}*(?:$|[{SPACE}.\-–—])")));

/// A possessive apostrophe that a filename or an OCR pass stripped:
/// "Lossky s Argument".
static MISSING_APOSTROPHE: LazyLock<Boundaried> =
    LazyLock::new(|| bounded(&format!(r"{WORD_START}[A-Za-z]{{2,}} s [A-Z]")));

/// Punctuation that a title has and a name does not.
static TITLE_PUNCTUATION: LazyLock<Regex> = LazyLock::new(|| pattern(r"[!?:]"));

/// A surname-first comma or a run of initials.
static INITIALS: LazyLock<Boundaried> =
    LazyLock::new(|| bounded(&format!(r"{WORD_START}[A-Z]\.{SPACE}*[A-Z]?\.?")));

/// Function words that essentially never appear in a personal name and are
/// everywhere in titles.
static TITLE_WORDS: LazyLock<Boundaried> = LazyLock::new(|| {
    bounded(&format!(
        r"(?i){WORD_START}(of|the|and|in|on|for|to|from|with|a|an|about|between|through|introduction|dictionary|handbook|companion|history|study|essays|conception|objections|architecture|philosophy|theory){WORD_END}"
    ))
});

/// How authors are joined when several share a book.
static AUTHOR_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| pattern(&format!(r"{SPACE}+&{SPACE}+|{SPACE}*;{SPACE}*")));

/// Publisher and imprint names that landed in the author field.
///
/// Matched whole — "Oxford University Press" as an author is wrong, but
/// "Oxford, Michael" is a person. Kept lowercase and compared against the
/// normalised field.
const PUBLISHER_AUTHORS: &[&str] = &[
    "baker academic",
    "blackwell",
    "bloomsbury",
    "brill",
    "cambridge",
    "cambridge university press",
    "continuum",
    "cornell university press",
    "crossway",
    "de gruyter",
    "eerdmans",
    "elsevier",
    "fortress press",
    "harpercollins",
    "harvard university press",
    "ignatius press",
    "intervarsity press",
    "macmillan",
    "mit press",
    "oxford",
    "oxford university press",
    "palgrave",
    "palgrave macmillan",
    "penguin",
    "penguin books",
    "princeton university press",
    "random house",
    "routledge",
    "sage",
    "simon schuster",
    "springer",
    "state university of new york press",
    "suny",
    "suny press",
    "t t clark",
    "university of chicago press",
    "wiley blackwell",
    "wipf stock",
    "yale university press",
    "zondervan",
];

/// Author fields that say there is no author.
const UNKNOWN_AUTHORS: &[&str] = &["", "anonymous", "n/a", "unknown", "unknown author"];

/// Word endings that mark a personal name.
const PERSON_SUFFIXES: &[&str] = &["ed", "eds", "ii", "iii", "iv", "jr", "md", "phd", "sr"];

/// What each fault is worth when the worst records are wanted first.
const WEIGHTS: &[(&str, u32)] = &[
    ("allcaps_title", 2),
    ("author_last_first", 1),
    ("author_role_suffix", 2),
    ("edited_volume", 1),
    ("filename_title", 3),
    ("junk_title", 3),
    ("lowercase_title", 2),
    ("missing_apostrophe", 2),
    ("numeric_author", 4),
    ("path_title", 4),
    ("placeholder_author", 4),
    ("placeholder_title", 4),
    ("possible_swap", 2),
    ("publisher_as_author", 3),
    ("sentence_case_title", 1),
    ("title_in_author", 3),
    ("underscore_title", 1),
    ("unknown_author", 3),
];

/// Everything wrong with a record, named.
///
/// `missing_isbn` is reported but is not on its own a reason to do anything:
/// nearly every book lacks one.
pub fn problems(title: Option<&str>, author: Option<&str>, isbn: Option<&str>) -> Vec<String> {
    let title = title.unwrap_or("").trim_matches(textkit::is_space);
    let author = author.unwrap_or("").trim_matches(textkit::is_space);
    let mut reasons = Vec::new();

    if UNKNOWN_AUTHORS
        .binary_search(&author.to_lowercase().as_str())
        .is_ok()
    {
        reasons.push("unknown_author");
    }
    if PLACEHOLDER_TITLE.is_match(title) {
        reasons.push("placeholder_title");
    }
    if PLACEHOLDER_AUTHOR.is_match(author) {
        reasons.push("placeholder_author");
    }
    if PATH_TITLE.is_match(title) {
        reasons.push("path_title");
    }
    if FILENAME_TITLE.is_match(title) {
        reasons.push("filename_title");
    }
    if JUNK_TITLE.is_match(title).unwrap_or(false) {
        reasons.push("junk_title");
    }
    if is_publisher(author) {
        reasons.push("publisher_as_author");
    }
    if NUMERIC_AUTHOR
        .find(author)
        .is_some_and(|at| at.start() == 0)
    {
        reasons.push("numeric_author");
    }
    if title.contains('_') {
        reasons.push("underscore_title");
    }
    if !title.is_empty() && title == title.to_uppercase() && title.contains(' ') {
        reasons.push("allcaps_title");
    }
    // A filename stub: "heidegger areopagite". Distinct from sentence case,
    // which is a much weaker signal.
    if !title.is_empty()
        && title == title.to_lowercase()
        && title.chars().any(|c| c.is_lowercase())
        && (title.contains(' ') || title.chars().count() >= 12)
    {
        reasons.push("lowercase_title");
    }
    if MISSING_APOSTROPHE.is_match(title).unwrap_or(false) {
        reasons.push("missing_apostrophe");
    }
    // Punctuation in the author field, or a long author against a short
    // title. Gated on the author not looking like a person, because several
    // authors joined together are many words against what may be a one-word
    // title, and that is not a swap.
    if !looks_like_person(author)
        && (TITLE_PUNCTUATION.is_match(author)
            || (author.split_whitespace().count() >= 5
                && (1..=3).contains(&title.split_whitespace().count())))
    {
        reasons.push("possible_swap");
    }
    if title_in_author(Some(title), Some(author)) {
        reasons.push("title_in_author");
    }
    if isbn
        .unwrap_or("")
        .trim_matches(textkit::is_space)
        .is_empty()
    {
        reasons.push("missing_isbn");
    }

    reasons.into_iter().map(str::to_string).collect()
}

/// How much attention a set of faults deserves.
pub fn severity(reasons: &[String]) -> u32 {
    reasons
        .iter()
        .filter_map(|reason| {
            WEIGHTS
                .binary_search_by_key(&reason.as_str(), |(name, _)| name)
                .ok()
                .map(|at| WEIGHTS[at].1)
        })
        .sum()
}

/// Whether anything here needs doing. A missing ISBN alone does not.
pub fn worth_fixing(reasons: &[String]) -> bool {
    reasons.iter().any(|reason| reason != "missing_isbn")
}

/// Whether the title field is a placeholder and the author field is holding
/// the real title.
///
/// Deliberately conservative: it must be both a placeholder title and an
/// author that does not read as a person, because what this drives — looking
/// the book up by the author text as a title — is wasted work otherwise.
pub fn title_in_author(title: Option<&str>, author: Option<&str>) -> bool {
    let title = title.unwrap_or("").trim_matches(textkit::is_space);
    let author = author.unwrap_or("").trim_matches(textkit::is_space);

    if author.is_empty() || !(PLACEHOLDER_TITLE.is_match(title) || PATH_TITLE.is_match(title)) {
        return false;
    }
    // A placeholder author is not a stray title. Treating it as one sends a
    // junk string to a catalogue as a title query, which is the spurious
    // match every other part of this guards against.
    if PLACEHOLDER_AUTHOR.is_match(author) {
        return false;
    }
    if is_publisher(author)
        || UNKNOWN_AUTHORS
            .binary_search(&author.to_lowercase().as_str())
            .is_ok()
    {
        return false;
    }
    !looks_like_person(author)
}

/// Whether a field reads as a human name.
///
/// Multi-author strings are judged one author at a time. A catalogue joins
/// authors with an ampersand, so two people arrive as one seven-word field —
/// judged whole it fails every test and reads as a title, which is how a
/// naive version of this flagged hundreds of perfectly good records.
pub fn looks_like_person(author: &str) -> bool {
    let author = author.trim_matches(textkit::is_space);
    if author.is_empty() {
        return false;
    }

    let parts: Vec<&str> = AUTHOR_SEPARATOR
        .split(author)
        .map(|part| part.trim_matches(textkit::is_space))
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() > 1 {
        return parts.iter().all(|part| looks_like_person(part));
    }

    let words: Vec<&str> = author.split_whitespace().collect();
    if author.contains(',') && words.len() <= 5 {
        return true; // "Surname, Given"
    }
    if INITIALS.is_match(author).unwrap_or(false) && words.len() <= 5 {
        return true;
    }
    if words.len() > 5 {
        return false;
    }
    if TITLE_WORDS.is_match(author).unwrap_or(false) {
        return false;
    }
    if words
        .iter()
        .any(|word| PERSON_SUFFIXES.contains(&word.to_lowercase().trim_matches('.')))
    {
        return true;
    }
    // A bare single word is a mononym far more often than a stray title —
    // Plato, Homer and Josephus are all over a philosophy library, and a
    // one-word "title" recovered from an author field is nearly never right.
    (1..=4).contains(&words.len())
}

/// A catalogue string reduced to the letters and digits in it.
pub fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        } else {
            space = true;
        }
    }
    out
}

fn is_publisher(author: &str) -> bool {
    PUBLISHER_AUTHORS
        .binary_search(&normalise(author).as_str())
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems_of(title: &str, author: &str) -> Vec<String> {
        problems(Some(title), Some(author), Some("9780198237907"))
    }

    #[test]
    fn the_tables_are_sorted_so_they_can_be_searched() {
        assert!(PUBLISHER_AUTHORS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(UNKNOWN_AUTHORS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(PERSON_SUFFIXES.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(WEIGHTS.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn a_good_record_has_nothing_wrong_with_it() {
        assert!(problems_of("The Human Condition", "Arendt, Hannah").is_empty());
    }

    #[test]
    fn a_missing_isbn_is_reported_but_is_not_worth_fixing() {
        let reasons = problems(Some("The Human Condition"), Some("Arendt, Hannah"), None);
        assert_eq!(reasons, ["missing_isbn"]);
        assert!(!worth_fixing(&reasons));
    }

    #[test]
    fn a_placeholder_title_is_found() {
        for title in ["Untitled", "untitled", "no title", "Document", "xxx", "N/A"] {
            assert!(
                problems_of(title, "Arendt, Hannah").contains(&"placeholder_title".to_string()),
                "{title}"
            );
        }
    }

    #[test]
    fn a_real_title_that_resembles_a_placeholder_is_left_alone() {
        for title in ["The Document", "A Test of Faith", "Untitled Symphony"] {
            assert!(
                !problems_of(title, "Arendt, Hannah").contains(&"placeholder_title".to_string()),
                "{title}"
            );
        }
    }

    #[test]
    fn a_filename_and_a_path_in_the_title_are_told_apart() {
        assert!(problems_of("chapter1.pdf", "x").contains(&"filename_title".to_string()));
        assert!(problems_of("A:\\pwecent2005.wpd", "x").contains(&"path_title".to_string()));
        assert!(problems_of("/home/matt/book", "x").contains(&"path_title".to_string()));
    }

    #[test]
    fn a_tool_name_in_the_title_is_junk() {
        for title in [
            "Microsoft Word - chapter3",
            "Book (z-lib.org)",
            "DjVu Document",
            "Full Page Fax Print",
            "scan OCR output",
        ] {
            assert!(
                problems_of(title, "x").contains(&"junk_title".to_string()),
                "{title}"
            );
        }
    }

    #[test]
    fn a_word_that_merely_contains_a_tool_name_is_not_junk() {
        assert!(!problems_of("Procrastination", "x").contains(&"junk_title".to_string()));
    }

    #[test]
    fn a_publisher_in_the_author_field_is_found() {
        assert!(
            problems_of("A Book", "Oxford University Press")
                .contains(&"publisher_as_author".to_string())
        );
        assert!(
            !problems_of("A Book", "Oxford, Michael").contains(&"publisher_as_author".to_string())
        );
    }

    #[test]
    fn shouting_and_whispering_are_both_noticed() {
        assert!(problems_of("THE HUMAN CONDITION", "x").contains(&"allcaps_title".to_string()));
        assert!(problems_of("heidegger areopagite", "x").contains(&"lowercase_title".to_string()));
        assert!(!problems_of("MIT", "x").contains(&"allcaps_title".to_string()));
    }

    #[test]
    fn several_authors_are_not_a_swapped_title() {
        let reasons = problems_of("Rama", "Arthur C. Clarke & Gentry Lee");
        assert!(
            !reasons.contains(&"possible_swap".to_string()),
            "{reasons:?}"
        );
    }

    #[test]
    fn a_title_in_the_author_field_is_found() {
        assert!(title_in_author(Some("Untitled"), Some("Arguing About War")));
        assert!(!title_in_author(Some("Untitled"), Some("Arendt, Hannah")));
        assert!(!title_in_author(
            Some("The Human Condition"),
            Some("Arguing About War")
        ));
        assert!(!title_in_author(Some("Untitled"), Some("Default")));
        assert!(!title_in_author(Some("Untitled"), Some("")));
    }

    #[test]
    fn people_and_titles_are_told_apart() {
        for person in [
            "Arendt, Hannah",
            "C. S. Lewis",
            "Plato",
            "Hannah Arendt",
            "C. S. Lewis & Madeleine L'Engle",
            "Smith Jr",
        ] {
            assert!(looks_like_person(person), "{person}");
        }
        for title in [
            "An Ethnohistorical Dictionary China",
            "Kierkegaard's Conception of God",
            "The Oxford Handbook of Something Or Other",
        ] {
            assert!(!looks_like_person(title), "{title}");
        }
    }

    #[test]
    fn severity_ranks_the_worst_first() {
        let bad = problems_of("Untitled", "Arguing About War");
        let mild = problems_of("Some_Book", "Arendt, Hannah");
        assert!(severity(&bad) > severity(&mild), "{bad:?} vs {mild:?}");
        assert_eq!(severity(&[]), 0);
    }

    #[test]
    fn normalising_keeps_only_what_identifies() {
        assert_eq!(
            normalise("Oxford University Press!"),
            "oxford university press"
        );
        assert_eq!(normalise("  T&T  Clark "), "t t clark");
        assert_eq!(normalise(""), "");
    }
}
