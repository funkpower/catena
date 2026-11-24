//! Recovering an author a record does not have.
//!
//! All of it local: what the book says about itself on its own first pages,
//! and what its title says. No lookup, no network, and nothing applied — a
//! name is proposed with the evidence that produced it and how far it is
//! worth trusting.

use std::sync::LazyLock;

use fancy_regex::Regex as Boundaried;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use textkit::pattern::{SPACE, WORD_END, WORD_START};

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

/// Words that mark a candidate as title or publisher bleed rather than a
/// person. Without them the looser heuristics propose the imprint.
static NOT_A_PERSON: LazyLock<Boundaried> = LazyLock::new(|| {
    Boundaried::new(&format!(
        r"(?i){WORD_START}(press|edition|introduction|university|volume|reprint|copyright|published|printed|rights|reserved|library|congress|isbn|inc|ltd|company|regents|trustees|foundation|institute|society|association|board|council|committee|department|ministry|editors?|trust|publishing|publishers|books|publications|sons|llc|gmbh|verlag|all|limited|house|media|editions|éditions|center|centre|seminary|classics|wiley|gruyter|brill|siebeck|seuil|gallimard|payot|routledge|springer|elsevier|macmillan|penguin|blackwell|norton|knopf|doubleday|vintage|bantam|harpercollins|nv|bv|microsoft|adobe|acrobat){WORD_END}"
    ))
    .expect("a valid pattern")
});

/// One token of a name: a capitalised word, a bare initial, or a nobiliary
/// particle that appears in the middle of one.
static NAME_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"^(?:[A-Z][A-Za-z’'\-]+\.?|[A-Z]\.|(?:von|van|de|den|der|del|della|di|da|la|le|du|ter|ten))$",
    )
});

/// Where several authors are joined.
static AUTHOR_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| pattern(&format!(r"(?i){SPACE}+(?:and|&){SPACE}+")));

/// "et al", "and others", and everything after.
static ET_AL: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(?i)[{SPACE},]+(?:et{SPACE}+al\.?|and{SPACE}+others).*$"
    ))
});

static SPACE_RUN: LazyLock<Regex> = LazyLock::new(|| pattern(&format!(r"{SPACE}+")));

/// An ISBN announced on the copyright page.
static ISBN: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"ISBN[:{SPACE}]*((?:97[89][{SPACE}-]?)?(?:[0-9][{SPACE}-]?){{9}}[0-9Xx])"
    ))
});

/// The strongest thing a book says about who wrote it.
///
/// The word "Copyright" is often left off: "© 2004 by Stanley Hauerwas".
static COPYRIGHT: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(?i)(?:Copyright{SPACE}*(?:©|\(c\))?|©|\(c\)){SPACE}*\d{{4}}{SPACE}+by{SPACE}+(.{{2,90}})"
    ))
});

/// What a notice can be for besides the work itself.
static SUBSIDIARY: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)\b(?:translat\w*|introduction|notes|foreword|afterword|preface|editorial|selection|compilation|illustrations?|cover|map|maps|abridg\w*|chronology)\b",
    )
});

/// The cataloguing block a national library prints on the copyright page.
static CATALOGUING: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r"(?i)Cataloging.{{0,40}}Publication(?:{SPACE}+Data)?"
    ))
});

/// Its main entry, which is the author, written surname first.
static MAIN_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"([A-Z][A-Za-z'’-]+),\s+([A-Z][A-Za-z.ّ'’ -]{1,40}?)[,.]"));

/// "… by Author Name" at the end of a title.
static TITLE_BY: LazyLock<Boundaried> = LazyLock::new(|| {
    Boundaried::new(&format!(
        r"(?i){WORD_START}by{SPACE}+([A-Z][A-Za-z.'’ -]{{2,60}})$"
    ))
    .expect("a valid pattern")
});

/// A title that opens with the author's name run together.
/// "Wilfred R Bion-Cogitations-Karnac Books (1991)": a filename that put the
/// author in front of the title with a hyphen, as shadow libraries name
/// their downloads. An optional series in brackets may come first, and the
/// names may be several, joined by commas or "and", marked "(Eds)".
static NAME_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    let name = r"[A-Z][\p{L}.'’]*(?:\s+[A-Z][\p{L}.'’]*){1,4}";
    pattern(&format!(
        r"^(?:\([^)]{{2,80}}\){SPACE}*)?(?P<names>{name}(?:{SPACE}*(?:,|\band\b|&){SPACE}*{name})*)(?:{SPACE}*\((?:Eds?|eds?)\.?{SPACE}*\))?{SPACE}*-{SPACE}*(?P<rest>\S.*)$"
    ))
});

/// The names a title opens with, and the title after them.
///
/// `None` unless every name reads as a person, so "Evidence-Based Technical
/// Analysis" and "Anglo-Saxon England" are left alone.
pub fn name_prefix_of_title(title: &str) -> Option<(Vec<String>, String)> {
    let found = NAME_PREFIX.captures(title.trim_matches(textkit::is_space))?;
    // "Socio-technical", "Non-Reductive": a hyphen inside a word, not after a
    // name. A title after a name starts with a capital; a number there is a
    // series counting its volumes ("Known Space-038-The Soft Weapon").
    if !found["rest"].chars().next().is_some_and(char::is_uppercase) {
        return None;
    }
    // "Can Physicalism Be Non-Reductive?", "Alfred Willener The Action-Image":
    // the words that make a run of capitals a sentence rather than a name.
    const NOT_IN_A_NAME: &[&str] = &[
        "a", "an", "are", "be", "can", "for", "how", "in", "is", "non", "of", "on", "the", "to",
        "towards", "what", "why", "with",
    ];
    if found["names"]
        .split_whitespace()
        .any(|word| NOT_IN_A_NAME.contains(&word.to_lowercase().as_str()))
    {
        return None;
    }
    let names: Vec<String> = AUTHOR_SEPARATOR
        .split(&found["names"].replace(" and ", " & ").replace(',', " &"))
        .map(|name| name.trim_matches(textkit::is_space).to_string())
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        return None;
    }
    let mut people = Vec::new();
    for name in &names {
        if NOT_A_PERSON.is_match(name).unwrap_or(true) {
            return None;
        }
        people.push(normalise(name)?);
    }
    Some((people, found["rest"].to_string()))
}

static TITLE_CAMEL: LazyLock<Regex> = LazyLock::new(|| pattern(r"^([A-Z][a-z]+[A-Z][a-z]+)[-\s]"));

static CAMEL_PARTS: LazyLock<Regex> = LazyLock::new(|| pattern(r"[A-Z][a-z]+"));

/// Roman numerals that are a suffix on a name rather than shouting.
const ROMAN_SUFFIXES: &[&str] = &["II", "III", "IV", "V", "VI"];

/// Characters trimmed from the ends of a candidate.
const EDGES: &[char] = &[' ', '.', ',', ';', ':', '—', '-', '\t', '\n'];

/// How far into a book's front matter to look.
pub const FRONT_MATTER_CHARS: usize = 8000;

/// Digits that are digits to Python and are not decimal.
///
/// A candidate name containing any digit is refused, and "any digit" there
/// means superscripts and circled numbers too — the ones a scanned page picks
/// up from a footnote marker.
const DIGIT_RANGES: &[(char, char)] = &[
    ('\u{b2}', '\u{b3}'),
    ('\u{b9}', '\u{b9}'),
    ('\u{1369}', '\u{1371}'),
    ('\u{19da}', '\u{19da}'),
    ('\u{2070}', '\u{2070}'),
    ('\u{2074}', '\u{2079}'),
    ('\u{2080}', '\u{2089}'),
    ('\u{2460}', '\u{2468}'),
    ('\u{2474}', '\u{247c}'),
    ('\u{2488}', '\u{2490}'),
    ('\u{24ea}', '\u{24ea}'),
    ('\u{24f5}', '\u{24fd}'),
    ('\u{24ff}', '\u{24ff}'),
    ('\u{2776}', '\u{277e}'),
    ('\u{2780}', '\u{2788}'),
    ('\u{278a}', '\u{2792}'),
    ('\u{10a40}', '\u{10a43}'),
    ('\u{10e60}', '\u{10e68}'),
    ('\u{11052}', '\u{1105a}'),
    ('\u{1f100}', '\u{1f10a}'),
];

/// A name, where it came from, and how far it is worth trusting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Suggestion {
    /// The name, normalised.
    pub author: String,
    /// Which signal produced it.
    pub method: String,
    /// How far to trust it, from zero to one.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub confidence: f64,
    /// The text it was read from, so a person can check it.
    pub evidence: String,
    /// An ISBN found alongside it, if there was one.
    pub isbn: Option<String>,
}

/// The best name a record's own text and title can offer, if any.
///
/// Candidates are gathered cheapest and most trustworthy first, and the most
/// confident wins. An ISBN found in the front matter is carried onto the
/// winner even when the winner came from the title, because the two are
/// facts about the same book.
pub fn best(title: &str, front_matter: Option<&str>) -> Option<Suggestion> {
    let mut candidates: Vec<Suggestion> = Vec::new();
    if let Some(text) = front_matter {
        candidates.extend(from_front_matter(text));
    }
    candidates.extend(from_title(title));

    let isbn = candidates
        .iter()
        .find_map(|candidate| candidate.isbn.clone());
    // The first of the most confident, which is the earliest and therefore
    // the cheapest signal that scored highest.
    let mut winner = candidates.into_iter().reduce(|best, candidate| {
        match candidate.confidence > best.confidence {
            true => candidate,
            false => best,
        }
    })?;
    if winner.isbn.is_none() {
        winner.isbn = isbn;
    }
    Some(winner)
}

/// Names a book's own front matter offers.
pub fn from_front_matter(text: &str) -> Vec<Suggestion> {
    let isbn = ISBN
        .captures(text)
        .and_then(|found| normalise_isbn(&found[1]));
    let mut out = Vec::new();

    // Strongest: the book saying who holds its copyright.
    // A notice for a translation, an introduction or the notes names whoever
    // wrote those, not the book: "Translation copyright © 2014 by Oliver
    // Ready" is in front of Dostoevsky.
    let for_the_work = COPYRIGHT.captures_iter(text).find(|found| {
        let at = found.get(0).map_or(0, |whole| whole.start());
        let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
        !SUBSIDIARY.is_match(&text[line_start..at])
    });
    if let Some(found) = for_the_work
        && let Some(name) = clean(&name_prefix(&found[1]))
        && name.split_whitespace().count() >= 2
    {
        // Two words at least: a lone surname is not an attribution.
        out.push(Suggestion {
            evidence: format!("…by {name}"),
            author: name,
            method: "front_matter_copyright".into(),
            confidence: 0.9,
            isbn: isbn.clone(),
        });
    }

    // The cataloguing block's main entry, written surname first. The
    // boilerplate is consumed first so the window starts at the name rather
    // than at the last word of the heading.
    if let Some(heading) = CATALOGUING.find(text) {
        let from = heading.end();
        let window: String = text[from..].chars().take(300).collect();
        if let Some(found) = MAIN_ENTRY.captures(&window)
            && let Some(name) = clean(&format!("{} {}", &found[2], &found[1]))
        {
            out.push(Suggestion {
                author: name,
                method: "front_matter_loc".into(),
                confidence: 0.8,
                evidence: found[0]
                    .trim_matches(textkit::is_space)
                    .chars()
                    .take(200)
                    .collect(),
                isbn,
            });
        }
    }
    out
}

/// Names the title string itself offers.
pub fn from_title(title: &str) -> Vec<Suggestion> {
    let mut out = Vec::new();

    if let Ok(Some(found)) = TITLE_BY.captures(title)
        && let Some(name) = clean(&found[1])
    {
        out.push(Suggestion {
            author: name,
            method: "title_by".into(),
            confidence: 0.7,
            evidence: found[0].trim_matches(textkit::is_space).to_string(),
            isbn: None,
        });
    }

    if let Some((names, _)) = name_prefix_of_title(title) {
        out.push(Suggestion {
            author: names.join(" & "),
            method: "title_prefix".into(),
            confidence: 0.75,
            evidence: title.chars().take(120).collect(),
            isbn: None,
        });
    }

    if let Some(found) = TITLE_CAMEL.captures(title)
        && let Some(split) = split_camel(&found[1])
    {
        out.push(Suggestion {
            author: split,
            method: "title_camel".into(),
            confidence: 0.5,
            evidence: found[1].to_string(),
            isbn: None,
        });
    }
    out
}

/// The leading run of name tokens, stopping at prose.
///
/// "James H. Billington Printed in the…" is "James H. Billington". The run is
/// capped so that a sentence which happens to start with capitals cannot be
/// swallowed whole, and a word ending in a full stop that is not an initial
/// ends the name — it is the end of a sentence.
pub fn name_prefix(text: &str) -> String {
    let mut tokens: Vec<&str> = Vec::new();
    for raw in text.split_whitespace() {
        let token = raw.trim_matches([',', ';', ':', '(', ')']);
        let core = token.trim_matches('.');
        if core.is_empty() || NOT_A_PERSON.is_match(core).unwrap_or(false) {
            break;
        }
        if !NAME_TOKEN.is_match(token) {
            break;
        }
        tokens.push(token);
        if token.ends_with('.') && core.chars().count() > 1 {
            break;
        }
        if tokens.len() >= 5 {
            break;
        }
    }
    // A particle belongs to the surname after it. Left on the end, it means
    // the run stopped at a firm's name: "Walter de" from "Walter de Gruyter".
    while tokens.last().is_some_and(|last| {
        [
            "de", "van", "von", "der", "den", "du", "da", "di", "la", "le",
        ]
        .contains(last)
    }) {
        tokens.pop();
    }
    tokens.join(" ")
}

/// Normalise a candidate, or refuse it if it does not read as a person.
pub fn clean(raw: &str) -> Option<String> {
    let name = SPACE_RUN.replace_all(raw, " ");
    let name = name.trim_matches(|c: char| EDGES.contains(&c));
    if name.is_empty() || name.chars().any(is_digit) {
        return None;
    }
    if NOT_A_PERSON.is_match(name).unwrap_or(false) {
        return None;
    }
    normalise(name)
}

/// Normalise a name, keeping co-authors joined.
///
/// `None` when any one author fails the shape a person's name has: one to
/// five words, capitalised, and not opening with an article.
pub fn normalise(raw: &str) -> Option<String> {
    let name = ET_AL.replace(raw, "");
    let name = name.trim_matches(textkit::is_space);

    let authors: Vec<String> = AUTHOR_SEPARATOR
        .split(name)
        .map(|segment| segment.trim_matches(textkit::is_space))
        .filter(|segment| !segment.is_empty())
        .map(normalise_tokens)
        .collect();
    if authors.is_empty() {
        return None;
    }

    for author in &authors {
        let words: Vec<&str> = author.split_whitespace().collect();
        if author.is_empty() || !(1..=5).contains(&words.len()) {
            return None;
        }
        let first = author.chars().next()?;
        if !first.is_uppercase() || words[0].to_lowercase() == "the" {
            return None;
        }
    }
    Some(authors.join(" & "))
}

/// Fix shouting and bare initials within one name.
fn normalise_tokens(segment: &str) -> String {
    segment
        .split_whitespace()
        .map(|token| {
            let core = token.trim_matches('.');
            let shouting = core.chars().all(char::is_alphabetic)
                && !core.is_empty()
                && core.chars().all(|c| !c.is_lowercase())
                && core.chars().any(char::is_uppercase)
                && core.chars().count() > 1
                && !ROMAN_SUFFIXES.contains(&core);
            if shouting {
                let mut fixed = capitalise(core);
                if token.ends_with('.') {
                    fixed.push('.');
                }
                return fixed;
            }
            if core.chars().count() == 1
                && token.chars().count() == 1
                && core.chars().all(|c| c.is_alphabetic() && c.is_uppercase())
            {
                return format!("{token}.");
            }
            token.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

/// A run-together name split back apart, when it is two or three parts.
fn split_camel(token: &str) -> Option<String> {
    let parts: Vec<&str> = CAMEL_PARTS
        .find_iter(token)
        .map(|found| found.as_str())
        .collect();
    let rejoined: String = parts.concat();
    ((2..=3).contains(&parts.len()) && rejoined == token).then(|| parts.join(" "))
}

fn normalise_isbn(raw: &str) -> Option<String> {
    let digits = crate::isbn::digits(raw);
    matches!(digits.len(), 10 | 13).then(|| digits.to_uppercase())
}

/// Whether a character is a digit, by Python's definition — which counts the
/// superscripts and the circled numbers that a decimal test does not.
fn is_digit(c: char) -> bool {
    // Not `is_ascii_digit`: a decimal digit in any script is a digit, and a
    // name carrying one is not a name whichever script it is written in.
    #[allow(clippy::is_digit_ascii_radix)]
    let decimal = c.is_digit(10);
    decimal
        || DIGIT_RANGES
            .iter()
            .any(|(from, to)| (*from..=*to).contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copyright_line_names_the_author() {
        let text = "Copyright © 1999 by James H. Billington\nPrinted in the United States";
        let found = from_front_matter(text);
        assert_eq!(found[0].author, "James H. Billington");
        assert_eq!(found[0].method, "front_matter_copyright");
        assert_eq!(found[0].confidence, 0.9);
    }

    #[test]
    fn a_notice_without_the_word_copyright_names_the_author() {
        let text = "\u{a9} 2004 by Stanley Hauerwas and Samuel Wells\n350 Main Street";
        let found = from_front_matter(text);
        assert_eq!(found[0].method, "front_matter_copyright");
        assert!(
            found[0].author.starts_with("Stanley Hauerwas"),
            "{}",
            found[0].author
        );
    }

    #[test]
    fn a_publisher_holding_the_copyright_is_not_an_author() {
        for text in [
            "Copyright \u{a9} 1990 by John Wiley & Sons",
            "\u{a9} 2004 by Random House, Inc.",
            "Copyright \u{a9} 2011 by Walter de Gruyter",
            "Copyright \u{a9} 2014 by Arktos Media Ltd.",
        ] {
            assert!(from_front_matter(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn a_translators_notice_is_passed_over() {
        let text = "Translation copyright \u{a9} 2014 by Oliver Ready\nIntroduction copyright \u{a9} 2014 by Someone Else";
        assert!(from_front_matter(text).is_empty());
        let text = "Translation copyright \u{a9} 2014 by Oliver Ready\nCopyright \u{a9} 1999 by James H. Billington";
        assert_eq!(from_front_matter(text)[0].author, "James H. Billington");
    }

    #[test]
    fn a_cataloguing_block_names_the_author_surname_first() {
        let text = "Library of Congress Cataloging-in-Publication Data\n\
                    Billington, James H., 1929-\n  The icon and the axe.";
        let found = from_front_matter(text);
        assert_eq!(found[0].author, "James H. Billington");
        assert_eq!(found[0].method, "front_matter_loc");
    }

    #[test]
    fn an_isbn_on_the_same_page_comes_with_the_name() {
        let text = "Copyright © 1999 by James Billington\nISBN 978-0-19-823790-7";
        assert_eq!(
            from_front_matter(text)[0].isbn.as_deref(),
            Some("9780198237907")
        );
    }

    #[test]
    fn a_title_that_names_its_author_offers_it() {
        let found = from_title("The Icon and the Axe by James Billington");
        assert_eq!(found[0].author, "James Billington");
        assert_eq!(found[0].method, "title_by");
    }

    #[test]
    fn a_run_together_name_at_the_front_of_a_title_is_split() {
        let found = from_title("CarrollQuigley Tragedy and Hope");
        assert_eq!(found[0].author, "Carroll Quigley");
        assert_eq!(found[0].confidence, 0.5);
    }

    #[test]
    fn the_most_confident_candidate_wins_and_keeps_the_isbn() {
        let front = "Copyright © 1999 by James Billington\nISBN 0198237901";
        let found = best("Tragedy and Hope by Carroll Quigley", Some(front)).expect("a name");
        assert_eq!(found.author, "James Billington");
        assert_eq!(found.isbn.as_deref(), Some("0198237901"));
    }

    #[test]
    fn an_isbn_alone_is_not_carried_anywhere() {
        // The ISBN rides along with a name the front matter produced. Front
        // matter that names nobody produces no candidate for it to ride on.
        let front = "ISBN 0198237901\nno attribution here";
        let found = best("Tragedy and Hope by Carroll Quigley", Some(front)).expect("a name");
        assert_eq!(found.method, "title_by");
        assert_eq!(found.isbn, None);
    }

    #[test]
    fn a_publisher_is_not_a_person() {
        assert_eq!(clean("Oxford University Press"), None);
        assert_eq!(clean("The Regents of the University"), None);
        assert!(from_front_matter("Copyright © 1999 by Cambridge University Press").is_empty());
    }

    #[test]
    fn a_name_with_a_number_in_it_is_refused() {
        assert_eq!(clean("James Billington 1929"), None);
        assert_eq!(clean("James\u{b2} Billington"), None);
    }

    #[test]
    fn shouting_and_bare_initials_are_fixed() {
        assert_eq!(clean("ARISTOTLE").as_deref(), Some("Aristotle"));
        assert_eq!(clean("Barbara A Kerr").as_deref(), Some("Barbara A. Kerr"));
        assert_eq!(clean("Smith Jr. II").as_deref(), Some("Smith Jr. II"));
    }

    #[test]
    fn co_authors_are_kept_and_joined() {
        assert_eq!(
            clean("John Schuster and John Sutton").as_deref(),
            Some("John Schuster & John Sutton")
        );
    }

    #[test]
    fn everything_after_et_al_is_dropped() {
        assert_eq!(clean("Smith et al.").as_deref(), Some("Smith"));
        assert_eq!(clean("Smith and others").as_deref(), Some("Smith"));
        // Refused before the trailing text is ever reached: a candidate with
        // a digit in it is not a name.
        assert_eq!(clean("Smith, and others, 1999"), None);
    }

    #[test]
    fn something_that_is_not_a_name_is_refused() {
        assert_eq!(clean(""), None);
        assert_eq!(clean("the icon and the axe"), None);
        assert_eq!(
            clean("A Very Long Name With Far Too Many Words In It"),
            None
        );
    }

    #[test]
    fn a_name_run_stops_at_prose() {
        assert_eq!(
            name_prefix("James H. Billington Printed in the United States"),
            "James H. Billington"
        );
        assert_eq!(name_prefix("Ludwig von Mises wrote"), "Ludwig von Mises");
    }

    #[test]
    fn nothing_to_go_on_offers_nothing() {
        assert_eq!(best("An Ordinary Title", None), None);
        assert_eq!(best("", Some("")), None);
    }
}
