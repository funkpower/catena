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
    /// Every checksum-valid ISBN printed on the page, the book's own first.
    ///
    /// A page that lists the other titles in a series prints each of their
    /// ISBNs once, often before the book's own; the book's own is printed
    /// again in its cataloguing block and on its copyright line. So the
    /// ISBNs are ranked by how often the page prints them, and the order
    /// printed only breaks a tie.
    pub isbns: Vec<String>,
    /// What the page says about an ISBN, where it says something: "vol. 2",
    /// "hardback", "ebook". Keyed by the ISBN.
    pub notes: Vec<(String, String)>,
}

static PUBLISHED_BY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*(?:first\s+)?published(?:\s+(?:in\s+)?(?:1[5-9]|20)\d{2})?\s+by\s+(?P<who>[^,.\n]{3,60})")
        .expect("a valid pattern")
});
static PRESS_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*(?P<who>[A-Z][^\n]{2,48}?(?:University Press|Press|Publishers|Publishing|Books|Verlag))\s*[,.]?\s*$")
        .expect("a valid pattern")
});
/// A sentence that merely mentions a press is not the press.
///
/// `Oxford is a registered trade mark of Oxford University Press` ends in a
/// publisher's name and is a legal notice.
static NOT_AN_IMPRINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)( is | are | was |registered|trade mark|trademark|imprint of|division of|©|\(c\)|@|;|\d{4})")
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

/// Every checksum-valid ISBN announced in a piece of text, the most often
/// printed first.
pub fn isbns(text: &str) -> Vec<String> {
    stated(text).into_iter().map(|(isbn, _)| isbn).collect()
}

/// The ISBNs a text announces, each with what is printed after it, ranked as
/// [`Imprint::isbns`] says.
fn stated(text: &str) -> Vec<(String, Option<String>)> {
    // (isbn, times printed, first printed at, note)
    let mut seen: Vec<(String, usize, usize, Option<String>)> = Vec::new();
    for caps in ISBN.captures_iter(text) {
        let digits = bibclean::isbn::digits(&caps[1]);
        if !(matches!(digits.len(), 10 | 13) && bibclean::isbn::valid(&digits)) {
            continue;
        }
        let end = caps.get(0).map_or(0, |whole| whole.end());
        let note = note_after(&text[end..]);
        match seen.iter_mut().find(|(isbn, ..)| *isbn == digits) {
            Some(entry) => {
                entry.1 += 1;
                if entry.3.is_none() {
                    entry.3 = note;
                }
            }
            None => seen.push((digits, 1, end, note)),
        }
    }
    seen.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
    seen.into_iter()
        .map(|(isbn, _, _, note)| (isbn, note))
        .collect()
}

/// "(vol. 2, doml 29: alk. paper)", "hardback", "(pbk.)": what follows an
/// ISBN on its line, up to the next ISBN.
fn note_after(rest: &str) -> Option<String> {
    let line = rest.split('\n').next().unwrap_or("");
    let line = match line.to_lowercase().find("isbn") {
        Some(at) => &line[..at],
        None => line,
    };
    let note = line
        .trim_start_matches(|c: char| c.is_whitespace() || c == '(' || c == '-' || c == '–')
        .split(')')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches([',', ';', '.', '–', '-'])
        .trim();
    let has_words = note.chars().filter(|c| c.is_alphabetic()).count() >= 2;
    (has_words && note.chars().count() <= 60).then(|| note.to_string())
}

/// Read the copyright page.
pub fn read(text: &str) -> Imprint {
    let found = stated(text);
    let mut out = Imprint {
        isbns: found.iter().map(|(isbn, _)| isbn.clone()).collect(),
        notes: found
            .into_iter()
            .filter_map(|(isbn, note)| note.map(|note| (isbn, note)))
            .collect(),
        ..Imprint::default()
    };

    if let Some(caps) = DESCRIPTION.captures(text) {
        out.place = Some(tidy(&caps["place"]));
        out.publisher = Some(tidy(&caps["who"]));
        out.year = caps["year"].parse().ok();
        return out;
    }

    out.publisher = PUBLISHED_BY
        .captures_iter(text)
        .chain(PRESS_LINE.captures_iter(text))
        .map(|caps| tidy(&caps["who"]))
        .find(|who| !NOT_AN_IMPRINT.is_match(who) && who.split_whitespace().count() <= 8);
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
                .captures(line)
                .map(|caps| tidy(&caps["city"]))
        });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_books_own_isbn_outranks_a_series_list() {
        let page = "Other titles in the series\n\
            THE CAMBRIDGE COMPANION TO CHRISTIAN DOCTRINE edited by Colin Gunton\n\
            ISBN 0 521 47118 4 hardback ISBN 0 521 47695 8 paperback\n\
            THE CAMBRIDGE COMPANION TO BIBLICAL INTERPRETATION edited by John Barton\n\
            ISBN 0 521 48144 9 hardback ISBN 0 521 48593 2 paperback\n\
            Library of Congress cataloguing in publication data\n\
            ISBN 0 521 48144 9 (hardback) – ISBN 0 521 48593 2 (paperback)\n\
            ISBN 0 521 48144 9 hardback\n";
        let found = read(page);
        assert_eq!(found.isbns[0], "0521481449");
        assert!(
            found
                .notes
                .contains(&("0521481449".to_string(), "hardback".to_string()))
        );
    }

    #[test]
    fn a_volumes_isbn_keeps_its_note() {
        let page = "ISBN 978-0-674-72666-6 (vol. 1, doml 28 : alk. paper)\n\
            ISBN 978-0-674-73083-0 (vol. 2, doml 29: alk. paper)\n";
        let found = read(page);
        assert_eq!(found.isbns, ["9780674726666", "9780674730830"]);
        assert_eq!(found.notes[1].1, "vol. 2, doml 29: alk. paper");
    }
}
