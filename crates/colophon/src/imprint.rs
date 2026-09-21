//! The copyright page: who published it, where, when, and under what ISBN.
//!
//! Everything on this page is formula. The same dozen sentences appear in
//! every book printed in English, which is what makes them worth matching
//! exactly rather than interpreting.

use std::sync::LazyLock;

use regex::Regex;

/// What the copyright page stated.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Imprint {
    /// The press.
    pub publisher: Option<String>,
    /// Where it publishes from, where the page named one place.
    pub place: Option<String>,
    /// The year of this edition, preferring first publication.
    pub year: Option<u16>,
    /// Every checksum-valid ISBN printed on the page, in the order printed.
    pub isbns: Vec<String>,
}

static PUBLISHED_BY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*(?:first\s+)?published(?:\s+(?:in\s+)?(?:1[5-9]|20)\d{2})?\s+by\s+(?P<who>[^,.\n]{3,60})")
        .expect("a valid pattern")
});
static PRESS_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*(?P<who>[A-Z][^\n]{2,58}?(?:University Press|Press|Publishers|Publishing|Books|Verlag))\s*[,.]?\s*$")
        .expect("a valid pattern")
});
static FIRST_PUBLISHED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)first published[^.\n]{0,40}?((?:1[5-9]|20)\d{2})").expect("a valid pattern")
});
static COPYRIGHT_YEAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:©|\(c\)|copyright)\s*(?:by\s+)?((?:1[5-9]|20)\d{2})")
        .expect("a valid pattern")
});
static ISBN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)isbn(?:[-\s]?1[03])?[:\s]*([0-9][0-9\s\-–—]{8,24}[0-9Xx])")
        .expect("a valid pattern")
});
/// `Oxford : Oxford University Press, 2007` — the labelled CIP's description,
/// which states all three at once.
static DESCRIPTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*Description\s*:\s*(?P<place>[^:|\n]{2,40}?)\s*:\s*(?P<who>[^,|\n]{3,60}?)\s*,\s*(?P<year>(?:1[5-9]|20)\d{2})")
        .expect("a valid pattern")
});

fn tidy(s: &str) -> String {
    s.trim()
        .trim_end_matches([',', '.', ';'])
        .trim()
        .to_string()
}

/// Every checksum-valid ISBN announced in a piece of text.
pub fn isbns(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for caps in ISBN.captures_iter(text) {
        let digits = bibclean::isbn::digits(&caps[1]);
        if matches!(digits.len(), 10 | 13)
            && bibclean::isbn::valid(&digits)
            && !out.contains(&digits)
        {
            out.push(digits);
        }
    }
    out
}

/// Read the copyright page.
pub fn read(text: &str) -> Imprint {
    let mut out = Imprint {
        isbns: isbns(text),
        ..Imprint::default()
    };

    if let Some(caps) = DESCRIPTION.captures(text) {
        out.place = Some(tidy(&caps["place"]));
        out.publisher = Some(tidy(&caps["who"]));
        out.year = caps["year"].parse().ok();
        return out;
    }

    out.publisher = PUBLISHED_BY
        .captures(text)
        .or_else(|| PRESS_LINE.captures(text))
        .map(|caps| tidy(&caps["who"]));
    out.year = FIRST_PUBLISHED
        .captures(text)
        .or_else(|| COPYRIGHT_YEAR.captures(text))
        .and_then(|caps| caps[1].parse().ok());
    out.place = text
        .lines()
        .map(str::trim)
        .find(|line| crate::title::PLACE.is_match(line) && line.split_whitespace().count() <= 4)
        .and_then(|line| {
            crate::title::PLACE
                .find(line)
                .map(|m| tidy(m.as_str().trim_end_matches(|c: char| !c.is_alphabetic())))
        });
    out
}
