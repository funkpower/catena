//! The opening of a book, cut into pages and each page named for what it is.
//!
//! Front matter is not prose and does not read like it. A title page is four
//! lines and no sentence; a copyright page is a dense block of formulae that
//! recur from publisher to publisher; a series advertisement is a list of
//! other people's books. Telling them apart is most of the work, because the
//! title that matters is on one of them and the titles that will mislead you
//! are on another.

use std::sync::LazyLock;

use regex::Regex;

/// What a page of front matter is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Nothing legible on it.
    Blank,
    /// Other books, advertised. The trap: it names two hundred works that
    /// are not this one, in the same shapes a title page uses.
    SeriesList,
    /// Publication facts: the imprint, the copyright, the ISBN, the CIP block.
    Copyright,
    /// The book's own table of contents.
    Contents,
    /// Few lines, no sentences — a title page, a half title, or a dedication.
    Title,
    /// Running text. The front matter is over.
    Text,
}

/// One page, cleaned of what the converter added.
#[derive(Debug, Clone)]
pub struct Page {
    /// Page number as the marker gave it, or position when there was none.
    pub number: u32,
    /// Legible lines, in order, with markup and typesetting slugs removed.
    pub lines: Vec<String>,
    /// The page's text as it arrived, for the block readers.
    pub text: String,
    /// What the page is.
    pub kind: Kind,
}

static PAGE_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*<!--\s*page\s+(\d+)\s*-->\s*$").expect("a valid pattern")
});
static IMAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"!\[[^\]]*\]\([^)]*\)").expect("a valid pattern"));
static HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#{1,6}\s+").expect("a valid pattern"));
/// `DESA01   19/9/06   6:02 PM   Page i` — the typesetter's slug, printed by
/// accident and carried by every page of some books.
static SLUG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[a-z]{2,10}[-_ ]?\d{1,6}\s+\d{1,2}/\d{1,2}/\d{2,4}.*page\s+[ivxlcdm\d]+\s*$")
        .expect("a valid pattern")
});
/// A converted EPUB's navigation: `[Begin Reading](dedication.xhtml#ded)`,
/// and the page links some converters leave behind.
static NAVIGATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(<|\[[^\]]*\]\([^)]*\)|https?://|www\.|previous page|next page|page_\w+)")
        .expect("a valid pattern")
});
static FOLIO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\d\s.\-–—ivxlcdmIVXLCDM|]+$").expect("a valid pattern"));
static BLANK_NOTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(this page (has been )?intentionally left blank|blank page)\.?$")
        .expect("a valid pattern")
});
static LIST_CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(available now|available soon|also by|also in this series|also available|\
other titles|other books|forthcoming|in the same series|praise for|advance praise|\
very short introductions|series editors?|titles in this series)",
    )
    .expect("a valid pattern")
});
static COPYRIGHT_CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(all rights reserved|first published|first edition|copyright\s*[©®(]?|[©®]\s*\d{4}|\(c\)\s*\d{4}|\
library of congress|british library|catalog(u)?ing[- ]in[- ]publication|\
printed (in|and bound)|isbn|no part of this (book|publication)|\
typeset (in|by)|a cip (record|catalogue))",
    )
    .expect("a valid pattern")
});
static CONTENTS_CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(table of )?contents$|\.{4,}\s*\d+\s*$").expect("a valid pattern")
});
/// A line of a series list: a work, then whoever wrote it, on one line.
static LISTED_WORK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Z][A-Za-z'’\-]*(?:[ ,][A-Z][A-Za-z'’\-]*){0,7}\s+[A-Z][a-z'’\-]+\s+[A-Z][A-Za-z'’\-]+\.?$")
        .expect("a valid pattern")
});
/// A person's name alone on a line.
pub static PERSON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[A-Z][A-Za-z'’\-\.]{0,20}\s+){1,3}[A-Z][A-Za-z'’\-]{1,25}\.?$")
        .expect("a valid pattern")
});

/// Letters spread across a line by a PDF's glyph positions, put back together.
///
/// `A N E W T R A N SL AT ION` is one word to a reader and eleven tokens to a
/// parser, and the line it ruins is usually the title.
pub fn despread(line: &str) -> String {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 4 {
        return line.to_string();
    }
    let singles = tokens.iter().filter(|t| t.chars().count() == 1).count();
    if singles * 2 < tokens.len() {
        return line.to_string();
    }
    let mut out: Vec<String> = Vec::new();
    let mut run = String::new();
    for token in tokens {
        if token.chars().count() <= 3 && token.chars().all(char::is_alphabetic) {
            run.push_str(token);
        } else {
            if !run.is_empty() {
                out.push(std::mem::take(&mut run));
            }
            out.push(token.to_string());
        }
    }
    if !run.is_empty() {
        out.push(run);
    }
    out.join(" ")
}

/// Is the line so damaged that reading it would invent something?
///
/// A font whose glyphs map into the private use area comes out of extraction
/// as a line of unassigned code points. It looks like text to a length check
/// and is not text.
fn illegible(line: &str) -> bool {
    let total = line.chars().filter(|c| !c.is_whitespace()).count();
    if total == 0 {
        return true;
    }
    let bad = line
        .chars()
        .filter(|c| {
            let n = *c as u32;
            (0xE000..=0xF8FF).contains(&n) || *c == '\u{FFFD}'
        })
        .count();
    bad * 5 > total
}

fn clean_line(raw: &str) -> Option<String> {
    let without_images = IMAGE.replace_all(raw, " ");
    let stripped = HEADING.replace(without_images.trim(), "");
    let line = despread(
        stripped
            .trim()
            .trim_matches(|c| c == '*' || c == '_' || c == '|')
            .trim(),
    );
    if line.is_empty()
        || illegible(&line)
        || SLUG.is_match(&line)
        || NAVIGATION.is_match(&line)
        || FOLIO.is_match(&line)
        || BLANK_NOTE.is_match(&line)
    {
        return None;
    }
    Some(line)
}

fn looks_like_prose(line: &str) -> bool {
    let words = line.split_whitespace().count();
    words > 14 || (words > 8 && line.trim_end().ends_with(['.', ';', ',']))
}

fn classify(lines: &[String], text: &str) -> Kind {
    if lines.is_empty() {
        return Kind::Blank;
    }
    if COPYRIGHT_CUE.find_iter(text).count() >= 2 {
        return Kind::Copyright;
    }
    if LIST_CUE.is_match(text) && lines.len() >= 5 {
        return Kind::SeriesList;
    }
    let listed = lines.iter().filter(|l| LISTED_WORK.is_match(l)).count();
    if lines.len() >= 6 && listed * 5 >= lines.len() * 2 {
        return Kind::SeriesList;
    }
    if lines.iter().take(3).any(|l| CONTENTS_CUE.is_match(l))
        || lines.iter().filter(|l| CONTENTS_CUE.is_match(l)).count() >= 4
    {
        return Kind::Contents;
    }
    if lines.len() <= 12 && !lines.iter().any(|l| looks_like_prose(l)) {
        return Kind::Title;
    }
    Kind::Text
}

/// Cut the opening into pages and name each one.
///
/// Page markers are used where the converter left them. Where it did not —
/// an EPUB has no pages to mark — the blank lines are the only boundary
/// there is, and blocks stand in for pages. That is weaker, which is why a
/// book converted from EPUB usually comes back with nothing but its ISBN:
/// the metadata is in its OPF, and [`bookmeta`] reads that.
///
/// [`bookmeta`]: https://git.pumpin.online/funkpower/catena
pub fn split(text: &str, budget: usize) -> Vec<Page> {
    let mut pages = Vec::new();
    if PAGE_MARKER.is_match(text) {
        let mut last_end = 0;
        let mut number = 0u32;
        for m in PAGE_MARKER.captures_iter(text) {
            let whole = m.get(0).expect("group zero is always there");
            if number > 0 || whole.start() > 0 {
                push(&mut pages, number, &text[last_end..whole.start()]);
            }
            number = m[1].parse().unwrap_or(number + 1);
            last_end = whole.end();
            if pages.len() > budget {
                break;
            }
        }
        push(&mut pages, number, &text[last_end..]);
    } else {
        for (i, block) in text.split("\n\n").enumerate() {
            push(&mut pages, i as u32 + 1, block);
            if pages.len() > budget {
                break;
            }
        }
    }
    pages.truncate(budget);
    continuations(&mut pages);
    pages
}

fn push(pages: &mut Vec<Page>, number: u32, text: &str) {
    let lines: Vec<String> = text.lines().filter_map(clean_line).collect();
    let kind = classify(&lines, text);
    pages.push(Page {
        number,
        lines,
        text: text.to_string(),
        kind,
    });
}

/// A series list runs over several pages and only the first one announces
/// itself. The pages after it are the same list and have to be refused too,
/// or the title taken off one of them is another book's.
fn continuations(pages: &mut [Page]) {
    for i in 1..pages.len() {
        if pages[i - 1].kind != Kind::SeriesList {
            continue;
        }
        let page = &pages[i];
        if page.kind == Kind::Blank || page.lines.len() < 6 {
            continue;
        }
        // Only a page that is itself a list continues one. Counting bare
        // names here would take a title page for advertising, because a
        // title set in capitals and broken across lines is, line by line,
        // the same shape as a person's name.
        let listed = page
            .lines
            .iter()
            .filter(|l| LISTED_WORK.is_match(l))
            .count();
        if page.lines.len() >= 6 && listed * 2 >= page.lines.len() {
            pages[i].kind = Kind::SeriesList;
        }
    }
}
