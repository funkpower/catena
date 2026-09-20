//! Reading one reference as far as its shape allows.
//!
//! Reference styles differ in punctuation and agree on order: who, when,
//! what, where. So the reading goes for the parts that announce themselves —
//! an identifier, a year in brackets, a title in italics or quotation marks —
//! and uses where those landed to divide up what is left. A field that does
//! not announce itself is left empty and named in `unsettled`, which is the
//! whole point: a wrong year and a real one look identical three stages
//! downstream, and a missing one does not.

use std::sync::LazyLock;

use regex::Regex;

use crate::{Id, Kind, Reference};

/// Shorter than this and it is a page number or a running head.
const MIN_ENTRY: usize = 12;

/// A reference that only points back at an earlier one.
static CONTINUATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[\s\p{Pi}\p{Pf}*_]*(?:ibid|idem|op\.\s*cit|loc\.\s*cit|op\.cit|loc\.cit)\b")
        .expect("a valid pattern")
});

static DOI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b10\.\d{4,9}/[^\s,;)\]>]+").expect("a valid pattern"));
static ARXIV: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\barxiv:\s*([A-Za-z\-.]*/?\d{4,7}(?:\.\d{4,5})?(?:v\d+)?)")
        .expect("a valid pattern")
});
static ISBN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bisbn(?:-?1[03])?:?\s*((?:97[89][\s\-]?)?(?:\d[\s\-]?){9}[\dXx])")
        .expect("a valid pattern")
});
static ISSN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bissn:?\s*(\d{4}[\s\-]?\d{3}[\dXx])").expect("a valid pattern")
});
static URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://[^\s,;>\]]+").expect("a valid pattern"));

/// A year inside brackets, whether alone or at the end of an imprint.
static PAREN_YEAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\((?:[^()]*?[^\d])?((?:1[4-9]|20)\d{2})[a-z]?\)").expect("a valid pattern")
});
/// A year after a comma, which is where an imprint puts it.
static COMMA_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[,(]\s*((?:1[4-9]|20)\d{2})[a-z]?\b").expect("a valid pattern"));
/// A year anywhere, which is the last resort.
static ANY_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b((?:1[4-9]|20)\d{2})\b").expect("a valid pattern"));

/// Pages announced as pages.
static PP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bpp?\.?\s*(\d{1,5}(?:\s*[-–—]+\s*\d{1,5})?)\b").expect("a valid pattern")
});
/// The page range a journal citation ends with: `12 (1998): 45-67`.
static COLON_PAGES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r":\s*(\d{1,5}(?:\s*[-–—]+\s*\d{1,5})?)\s*\.?\s*$").expect("a valid pattern")
});
/// A page range after a comma or colon, which is where author-date styles
/// put one: `Nature, 570(7761), 321-325`.
static RANGE_PAGES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[,:]\s*(\d{1,5}\s*[-–—]+\s*\d{1,5})(?:\b|$)").expect("a valid pattern")
});
/// The bare page a note ends with: `…, 183.`
static TRAILING_PAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r",\s*(\d{1,5})\s*\.?\s*$").expect("a valid pattern"));

/// A title in markdown emphasis, or in the tags a converter left instead.
static EMPHASISED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\*([^*\n]{3,300})\*|_([^_\n]{3,300})_|<(?:em|i|cite)\b[^>]*>(.{3,300}?)</(?:em|i|cite)>")
        .expect("a valid pattern")
});
/// A title in quotation marks, straight or curly.
static QUOTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""([^"\n]{3,300})"|“([^”\n]{3,300})”|«([^»\n]{3,300})»|‘([^’\n]{3,300})’"#)
        .expect("a valid pattern")
});

/// The imprint of a book: a place, a colon, a publisher.
static IMPRINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[(.,;]\s*)[\p{Lu}][^:;()\d]{1,40}:\s*([^;()]{2,80})")
        .expect("a valid pattern")
});

/// Words that name a publisher rather than a journal.
static PUBLISHER_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:press|publisher|publishing|books|verlag|editions?|university|imprint|&\s*co\b|ltd\b|inc\b)")
        .expect("a valid pattern")
});

/// The volume-and-number a journal citation carries.
static VOLUME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:vol\.?\s*)?\d{1,4}\s*(?:,\s*)?(?:\(\s*\d+\s*\)|no\.\s*\d+|\(\s*(?:1[4-9]|20)\d{2}\s*\))")
        .expect("a valid pattern")
});

/// What marks a piece inside a larger edited work.
static WITHIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[,.]\s*)in\s|\bed(?:s|ited by)?\.?\s").expect("a valid pattern")
});

/// The role a compiler of a book is credited with, at the end of the run of
/// names it follows.
static ROLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i),?\s+(?:ed|eds|edited\s+by|trans|translated\s+by|comp|comps|rev)\.?$")
        .expect("a valid pattern")
});

/// A run of text that is nothing but an identifier.
static IDENTIFIER_ONLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:arxiv:|doi:|https?://|isbn|issn|\b10\.\d{4})\S*\.?$")
        .expect("a valid pattern")
});

/// Any HTML tag, which conversion scatters through a reference list.
static MARKUP_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"</?[a-zA-Z][^>]*>").expect("a valid pattern"));

/// The Chicago note: a name, the title, the imprint in brackets, the page.
/// Nothing in it is marked up, so the commas and the brackets are all there
/// is to divide it by.
static NOTE_FORM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^,()]{2,60}),\s+([^()]{4,300}?)\s*\(([^()]*(?:1[4-9]|20)\d{2}[a-z]?)\)")
        .expect("a valid pattern")
});

/// What a note says before it gets to the citation.
static LEAD_IN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:see|cf\.|compare|quoted|cited|as\s+quoted|reprinted)(?:\s*,?\s*(?:also|for\s+(?:example|instance)|e\.g\.|in))*\s*,?\s+")
        .expect("a valid pattern")
});

/// The lowercase words a name may contain. Anything else lowercase in the
/// middle of a supposed name means it is a sentence.
const PARTICLES: [&str; 20] = [
    "van", "von", "de", "del", "della", "der", "den", "di", "da", "du", "la", "le", "of", "y",
    "bin", "ibn", "al", "ter", "v", "zu",
];

/// Anything that says this is a reference at all.
static CITATION_SIGNAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:1[4-9]|20)\d{2}\b|\bpp?\.\s*\d|https?://|\b10\.\d{4}|\bed\.|\btrans\.|\bvol\.|\bno\.|:\s*\d|\bpress\b|\buniversity\b|\bjournal\b")
        .expect("a valid pattern")
});

/// A rule of dashes standing in for the author of the entry above, which is
/// how every style avoids printing the same name forty times running.
static REPEATED_AUTHOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:[—–]+|-{3,}|_{3,})").expect("a valid pattern"));

/// Whether an entry defers to the one above it for its author.
pub fn repeats_author(text: &str) -> bool {
    REPEATED_AUTHOR.is_match(text)
}

/// Read one entry.
///
/// `None` when it is not a reference at all — a section heading, a page
/// number, a stray line of running text that the cut picked up.
pub fn one(text: &str, marker: Option<&str>) -> Option<Reference> {
    let text = text.trim();
    if text.chars().count() < MIN_ENTRY {
        return None;
    }

    let mut out = Reference::of(text);
    out.marker = marker.map(str::to_string);

    if CONTINUATION.is_match(text) {
        out.continuation = true;
        out.pages = pages(text);
        return Some(out);
    }
    if !CITATION_SIGNAL.is_match(text) {
        return None;
    }

    out.ids = ids(text);
    out.year = year(text);
    out.pages = pages(text);

    let (quoted, quoted_at) = span(&QUOTED, text);
    let (emphasised, emphasised_at) = span(&EMPHASISED, text);
    let dated = author_date(text);

    // An author-date opening settles the authors and the year whatever the
    // rest of the entry marks up, so it is asked first. A bibliography full
    // of German titles has quotation marks inside its titles, and taking
    // those for the title is how a parser that asked the other way round
    // gets it wrong.
    if let Some(year_at) = dated {
        out.authors = authors(&text[..year_at.0]);
        let tail = text[year_at.1..].trim_start_matches(|c: char| ".,:‘“\" ".contains(c));
        let tail_at = text.len() - tail.len();
        // Quotation marks count as the title only where they open the tail.
        // Further in they are a quotation inside a title, not the title.
        let opens_tail = quoted_at.is_some_and(|at| at.0 + 2 >= tail_at && at.0 <= tail_at + 2);
        let after_year = |at: (usize, usize)| at.0 >= year_at.1;

        if opens_tail {
            out.title = quoted;
            let at = quoted_at.expect("a span");
            out.container = match (emphasised, emphasised_at) {
                (Some(book), Some(book_at)) if book_at.0 >= at.1 => tidy_container(&book),
                _ => container(text, &text[at.1..]),
            };
        } else {
            let parts = sentences(tail);
            out.title = parts.first().map(|first| tidy_title(&strip_markup(first)));
            out.container = match (emphasised, emphasised_at) {
                (Some(journal), Some(at)) if after_year(at) => tidy_container(&journal),
                _ => parts.get(1).and_then(|second| tidy_container(second)),
            };
        }
    } else if let (Some(title), Some(at)) = (quoted, quoted_at) {
        out.title = Some(title);
        out.authors = authors(&text[..at.0]);
        // A piece inside an edited book appeared in that book, and the book
        // is the italic title after this one.
        let within = WITHIN.is_match(&text[at.1..]);
        out.container = match (within, emphasised, emphasised_at) {
            (true, Some(book), Some(book_at)) if book_at.0 >= at.1 => Some(book),
            _ => container(text, &text[at.1..]),
        };
    } else if let (Some(title), Some(at)) = (emphasised, emphasised_at) {
        out.title = Some(title);
        out.authors = authors(&text[..at.0]);
        out.container = container(text, &text[at.1..]);
    } else if let Some(found) = note_form(text).or_else(|| comma_form(text)) {
        out.authors = found.0;
        out.title = found.1;
        out.container = found.2;
    } else {
        let (found_authors, found_title, found_container) = by_sentence(text);
        out.authors = found_authors;
        out.title = found_title;
        out.container = found_container;
    }
    out.kind = kind(text, &out);

    Some(out)
}

/// The first match of a title pattern: what it held, and where it sat.
fn span(pattern: &Regex, text: &str) -> (Option<String>, Option<(usize, usize)>) {
    let Some(found) = pattern.captures(text) else {
        return (None, None);
    };
    let whole = found.get(0).expect("the whole match");
    let Some(inner) = (1..=4).find_map(|at| found.get(at)) else {
        return (None, None);
    };
    let cleaned = tidy_title(&strip_markup(inner.as_str()));
    match cleaned.is_empty() {
        true => (None, None),
        false => (Some(cleaned), Some((whole.start(), whole.end()))),
    }
}

/// An author-date opening with no brackets round the year: `Altmann, M.
/// 1951, A Study of Behaviour`. Common in the sciences and in older German
/// and Russian bibliographies.
static NAME_YEAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.{2,80}?)[,.]?\s+((?:1[4-9]|20)\d{2})[a-z]?\s*[,.:]\s+")
        .expect("a valid pattern")
});

/// Where an author-date entry puts its year, when it is one.
///
/// The bracketed year has to come early and what precedes it has to read as
/// names — otherwise `(Oxford, 1998)` at the end of a note would look like
/// the opening of an author-date entry with the whole note as its authors.
fn author_date(text: &str) -> Option<(usize, usize)> {
    let span = match PAREN_YEAR.find(text) {
        Some(found) => (found.start(), found.end()),
        None => {
            let found = NAME_YEAR.captures(text)?;
            let year = found.get(2).expect("the year");
            let whole = found.get(0).expect("the whole match");
            (year.start(), whole.end())
        }
    };
    let head = &text[..span.0];
    if head.chars().count() > 120 || !reads_as_names(head) {
        return None;
    }
    // Something has to follow the year, or there is no title to find.
    match text[span.1..].trim().chars().count() >= 4 {
        true => Some(span),
        false => None,
    }
}

/// Read a note that marked none of its parts but bracketed its imprint.
///
/// `Name, Title (Place: Publisher, Year), page.` — the commonest shape in
/// any book with endnotes, and the one that offers a parser nothing but its
/// punctuation. The guard is that the part standing where the title goes has
/// to be one sentence: a contributor's biography opens the same way and runs
/// on for three.
fn note_form(text: &str) -> Option<(Vec<String>, Option<String>, Option<String>)> {
    let text = LEAD_IN.replace(text.trim(), "");
    let found = NOTE_FORM.captures(&text)?;
    let names = authors(&found[1]);
    if names.is_empty() {
        return None;
    }
    let middle = found[2].trim();
    if sentences(middle).len() > 1 {
        return None;
    }
    let title = tidy_title(&strip_markup(middle));
    if title.chars().count() < 4 {
        return None;
    }
    Some((names, Some(title), tidy_container(&found[3])))
}

/// Read an entry whose only punctuation is commas.
///
/// `Miller, David, On Nationality, Oxford: Clarendon Press, 1995` — the style
/// gives nothing away except that the imprint has a colon in it, so the title
/// is taken to be whatever sits between the names and the imprint. Without an
/// imprint there is no second edge and nothing to be read here.
fn comma_form(text: &str) -> Option<(Vec<String>, Option<String>, Option<String>)> {
    let text = LEAD_IN.replace(text.trim(), "");
    if sentences(&text).len() > 1 {
        return None;
    }
    let parts: Vec<&str> = text.split(',').collect();
    if parts.len() < 4 {
        return None;
    }
    // The imprint is whichever comma-part carries the colon. Found by
    // counting commas rather than by where a pattern matched: a pattern for
    // `Place: Publisher` starts at the comma before the place, which is
    // exactly where the title ends, and the two cannot both have it.
    let at = parts.iter().position(|part| part.contains(':'))?;

    // `Miller, David` is the usual opening, and `Procopius` on its own is the
    // other one. Whichever reads as a name, the title starts after it.
    let pair = format!("{},{}", parts[0], parts[1]);
    let (names, used) = match reads_as_names(&pair) {
        true => (authors(&pair), 2),
        false if reads_as_names(parts[0]) => (authors(parts[0]), 1),
        false => return None,
    };
    if names.is_empty() || at <= used {
        return None;
    }

    let title = tidy_title(&strip_markup(&parts[used..at].join(",")));
    if title.chars().count() < 4 {
        return None;
    }
    Some((names, Some(title), tidy_container(parts[at])))
}

/// Whether a run of text is names and nothing else.
///
/// Stricter than asking whether it *contains* a name. `Doe, Jane, and Peter
/// Brown. "Obligation and Its Limits." Journal of Medieval Studies 12, no. 3`
/// contains one, and reading it as the authors of an author-date entry would
/// take the journal's issue number for a year.
fn reads_as_names(head: &str) -> bool {
    let head = head.trim_end_matches(|c: char| ",.;: ".contains(c));
    if head.contains(['"', '“', '”', '*', '_', '(', ')']) {
        return false;
    }
    let head = ROLE.replace(head, "");
    let parts = split_parts(&head);
    !parts.is_empty() && parts.iter().all(|part| is_name(part))
}

/// Drop the emphasis and the tags a converter leaves round a title.
fn strip_markup(text: &str) -> String {
    let without_tags = MARKUP_TAG.replace_all(text, "");
    without_tags.replace(['*', '_'], "").trim().to_string()
}

/// Every identifier the entry carries, normalised.
///
/// Order is fixed rather than found: two entries carrying the same
/// identifiers produce the same record whichever way round they wrote them.
fn ids(text: &str) -> Vec<Id> {
    let mut out = Vec::new();
    let mut push = |kind: &str, raw: &str| {
        let normalised = match kind {
            "url" => Some(raw.trim_end_matches(['.', ',', ';', ')']).to_string()),
            _ => idnorm::parse(raw).map(|found| found.id),
        };
        if let Some(id) = normalised {
            let one = Id {
                kind: kind.to_string(),
                id,
            };
            if !out.contains(&one) {
                out.push(one);
            }
        }
    };

    for found in DOI.find_iter(text) {
        push("doi", found.as_str().trim_end_matches(['.', ',', ';']));
    }
    for found in ARXIV.captures_iter(text) {
        push("arxiv", &found[1]);
    }
    for found in ISBN.captures_iter(text) {
        push("isbn", &found[1]);
    }
    for found in ISSN.captures_iter(text) {
        push("issn", &found[1]);
    }
    for found in URL.find_iter(text) {
        // A DOI written as a URL is already recorded as the DOI it is.
        if !found.as_str().contains("doi.org") && !found.as_str().contains("arxiv.org") {
            push("url", found.as_str());
        }
    }
    out
}

/// The year of publication.
///
/// A bracketed year first, because that is where a style puts the one it
/// means; then a year after a comma, which is where an imprint ends; then
/// any year at all, which may well be part of a title.
fn year(text: &str) -> Option<u32> {
    for pattern in [&*PAREN_YEAR, &*COMMA_YEAR, &*ANY_YEAR] {
        if let Some(found) = pattern.captures(text) {
            return found[1].parse().ok();
        }
    }
    None
}

/// The pages, as the entry wrote them.
///
/// The last two patterns end at a number, and so does an imprint — so a
/// value that reads as a year is refused rather than reported as a page. A
/// book published in 1998 is not a book cited at page 1998.
fn pages(text: &str) -> Option<String> {
    for pattern in [&*PP, &*COLON_PAGES, &*RANGE_PAGES, &*TRAILING_PAGE] {
        let Some(found) = pattern.captures(text) else {
            continue;
        };
        let value: String = found[1].split_whitespace().collect::<Vec<_>>().join("");
        if value.split(['-', '–', '—']).all(looks_like_a_year) {
            continue;
        }
        return Some(value);
    }
    None
}

/// Whether a number is in the range a year of publication falls in.
fn looks_like_a_year(text: &str) -> bool {
    match text.parse::<u32>() {
        Ok(number) => (1400..=2099).contains(&number),
        Err(_) => false,
    }
}

/// Strip the punctuation a style leaves inside its own title delimiters.
fn tidy_title(raw: &str) -> String {
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    text.trim_matches(|c: char| ",.;:".contains(c) || c.is_whitespace())
        .to_string()
}

/// Who wrote it, from the run of text before the title.
fn authors(before: &str) -> Vec<String> {
    let mut head = LEAD_IN.replace(before.trim(), "").trim().to_string();
    let mut head = head.as_mut_str() as &str;
    // An author-date entry puts the year between the authors and the title.
    if let Some(at) = head.rfind('(') {
        if head[at..].contains(|c: char| c.is_ascii_digit()) {
            head = head[..at].trim();
        }
    }
    let head = head.trim_end_matches(|c: char| ",.;:".contains(c) || c.is_whitespace());
    // `Eberhardt, Christian, ed.` is an editor, and an editor is a person.
    // The role is dropped rather than the name with it.
    split_authors(&ROLE.replace(head, ""))
}

/// Cut a run of names at the separators a reference list uses.
fn split_authors(head: &str) -> Vec<String> {
    split_parts(head)
        .into_iter()
        .filter(|part| is_name(part))
        .collect()
}

/// The separators a list of authors is joined by.
///
/// A bare comma is not among them: `Smith, John` is one author and
/// `Smith, John, and Jane Doe` is two, so only the words can divide them.
fn split_parts(head: &str) -> Vec<String> {
    let head = head.trim();
    if head.is_empty() || head.chars().count() > 200 {
        return Vec::new();
    }
    let mut parts: Vec<String> = vec![head.to_string()];
    for separator in [", and ", " and ", "; ", " & ", ", & "] {
        if head.contains(separator) {
            parts = head
                .split(separator)
                .map(|part| part.trim().to_string())
                .collect();
            break;
        }
    }

    parts
        .into_iter()
        .map(|part| {
            // A trailing "et al." means the entry dropped the rest itself,
            // and there is nothing here to recover.
            restore_initial(
                part.trim_end_matches(|c: char| ",.;".contains(c) || c.is_whitespace())
                    .trim_end_matches("et al")
                    .trim_end_matches(|c: char| ",.;".contains(c) || c.is_whitespace()),
            )
        })
        .filter(|part| !part.is_empty())
        .collect()
}

/// Put back the full stop that belongs to a closing initial.
///
/// Trimming trailing punctuation is right for `Smith, John,` and wrong for
/// `Peirce, C. S.`, where the stop is part of the initial and not the
/// separator that followed it.
fn restore_initial(name: &str) -> String {
    let last = name
        .rsplit(|c: char| c.is_whitespace())
        .next()
        .unwrap_or("");
    match last.chars().count() == 1 && last.chars().all(char::is_uppercase) {
        true => format!("{name}."),
        false => name.to_string(),
    }
}

/// Whether a run of words reads as somebody's name.
fn is_name(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() || words.len() > 6 {
        return false;
    }
    if text.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    // An editor's or translator's credit is a role, not a name.
    if text.to_lowercase().contains("ed.") || text.to_lowercase().contains("trans.") {
        return false;
    }
    // A lowercase word in the middle of a name is a particle or a sentence,
    // and there are only so many particles.
    let sentence = words.iter().any(|word| {
        let core = word.trim_matches(|c: char| !c.is_alphanumeric());
        core.chars().next().is_some_and(char::is_lowercase)
            && !PARTICLES.contains(&core.to_lowercase().as_str())
    });
    if sentence {
        return false;
    }
    words
        .first()
        .is_some_and(|word| word.chars().next().is_some_and(char::is_uppercase))
}

/// What the work appeared in: a journal, or a book's publisher.
fn container(_whole: &str, after: &str) -> Option<String> {
    // A book names its publisher in an imprint after the title, whether
    // bracketed or not. Only after the title: a title with a colon in it —
    // and half of them have one — would otherwise read as an imprint.
    if let Some(found) = IMPRINT.captures(after) {
        if let Some(publisher) = tidy_container(found[1].trim()) {
            return Some(publisher);
        }
    }
    // A journal sits between the title and the volume.
    let tail = after.trim_start_matches(|c: char| ",.;:\"”'’ ".contains(c));
    let end = VOLUME
        .find(tail)
        .map(|found| found.start())
        .or_else(|| tail.find(['(', '.', ';']))
        .unwrap_or(tail.len());
    tidy_container(&tail[..end])
}

/// Reduce a run of text to the name of the thing a work appeared in.
///
/// What arrives is a whole trailing segment — the journal with its volume,
/// issue and pages after it, or the imprint with its year. All of those are
/// recorded in their own fields already, so what is wanted here is only the
/// name, and everything that is somewhere else on the record is cut away.
fn tidy_container(raw: &str) -> Option<String> {
    let text = strip_markup(raw);
    // An identifier is not a container. A preprint has no journal, and
    // saying `arXiv:1706.03762` where a journal goes invents one.
    if IDENTIFIER_ONLY.is_match(text.trim()) {
        return None;
    }
    let mut text = text.as_str();
    for pattern in [&*VOLUME, &*RANGE_PAGES, &*COMMA_YEAR, &*PP] {
        if let Some(found) = pattern.find(text) {
            text = &text[..found.start()];
        }
    }
    let text = tidy_title(text);
    let text = text.trim_end_matches(|c: char| ",.;: ".contains(c));
    // `Oxford: Clarendon Press` names a place and then a publisher, and it
    // is the publisher a catalogue is keyed on. The place stays in `text`.
    let text = match text.split_once(": ") {
        Some((place, publisher))
            if place.chars().count() <= 40
                && !place.chars().any(|c| c.is_ascii_digit())
                && publisher.chars().count() >= 3 =>
        {
            publisher
        }
        _ => text,
    };
    // `Sociometry 14` is a journal and a volume, and the volume is a field
    // this record does not keep. What it is not is part of the name.
    let text = match text.rsplit_once(' ') {
        Some((name, last))
            if name.chars().count() >= 3
                && last.chars().all(|c| c.is_ascii_digit())
                && !last.is_empty() =>
        {
            name.trim_end_matches(',')
        }
        _ => text,
    };
    match text.chars().count() >= 3 && text.chars().any(char::is_alphabetic) {
        true => Some(text.to_string()),
        false => None,
    }
}

/// Divide an entry that marked none of its parts, on its sentence stops.
///
/// The order is the one every style agrees on: who, then what, then where.
/// It is weaker than reading a marked-up title and is only reached when the
/// entry offered nothing to read.
fn by_sentence(text: &str) -> (Vec<String>, Option<String>, Option<String>) {
    let head = match PAREN_YEAR.find(text) {
        // Author-date: the authors are what comes before the bracketed year.
        Some(found) => &text[..found.start()],
        None => text,
    };
    let segments: Vec<&str> = sentences(text);
    let found_authors = match PAREN_YEAR.find(text) {
        Some(_) => authors(head),
        None => segments
            .first()
            .map(|first| authors(first))
            .unwrap_or_default(),
    };

    // One segment and no names in it means the entry was never divided: the
    // commas that hold it together say nothing about where the title starts.
    // Reporting the whole entry as the title would be a wrong answer dressed
    // as a settled one, which is worse than no answer at all.
    if segments.len() < 2 && found_authors.is_empty() {
        return (found_authors, None, None);
    }

    // The title is the first segment that is not the authors and is not an
    // imprint. An imprint has a colon in it, or names a publisher.
    let mut rest = segments.iter().skip(match found_authors.is_empty() {
        true => 0,
        false => 1,
    });
    let title = rest
        .by_ref()
        .find(|segment| {
            let segment = segment.trim();
            segment.chars().count() >= 4 && !PAREN_YEAR.is_match(segment)
        })
        .map(|segment| tidy_title(segment));
    let container = rest
        .find(|segment| {
            let segment = segment.trim();
            segment.chars().count() >= 3 && segment.chars().any(char::is_alphabetic)
        })
        .and_then(|segment| tidy_container(segment));
    (found_authors, title, container)
}

/// Cut an entry at its sentence stops, which is where styles put their parts.
///
/// A stop inside an initial — `J. R. R.` — or after a known abbreviation is
/// not a stop, which is why this is not a split on '.'.
fn sentences(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'.' {
            let before = text[start..at].trim_end();
            let last = before
                .rsplit(|c: char| c.is_whitespace())
                .next()
                .unwrap_or("");
            let initial = last.chars().count() <= 1;
            let abbreviated = matches!(
                last.to_lowercase().as_str(),
                "ed" | "eds"
                    | "trans"
                    | "vol"
                    | "no"
                    | "pp"
                    | "p"
                    | "rev"
                    | "st"
                    | "mr"
                    | "mrs"
                    | "dr"
                    | "jr"
                    | "sr"
                    | "cf"
                    | "esp"
                    | "repr"
            );
            let followed_by_space = bytes.get(at + 1).is_some_and(|c| *c == b' ');
            if !initial && !abbreviated && followed_by_space {
                out.push(text[start..at].trim());
                start = at + 1;
            }
        }
        at += 1;
    }
    if start < text.len() {
        let tail = text[start..].trim();
        if !tail.is_empty() {
            out.push(tail);
        }
    }
    out.into_iter().filter(|part| !part.is_empty()).collect()
}

/// What sort of work the entry points at.
fn kind(text: &str, found: &Reference) -> Kind {
    if found.ids.iter().any(|id| id.kind == "arxiv") {
        return Kind::Article;
    }
    let quoted = QUOTED.is_match(text);
    let within = WITHIN.is_match(text);
    if quoted && within {
        return Kind::Chapter;
    }
    if found
        .ids
        .iter()
        .any(|id| id.kind == "doi" || id.kind == "issn")
    {
        return Kind::Article;
    }
    if quoted && VOLUME.is_match(text) {
        return Kind::Article;
    }
    // An imprint — a place, a colon, a publisher — is what a book has and a
    // journal article does not. Asked of the entry rather than of the
    // container, because the container has had the place cut off it by then.
    let publisher = IMPRINT.is_match(text)
        || found
            .container
            .as_deref()
            .is_some_and(|container| PUBLISHER_WORD.is_match(container));
    if publisher {
        return match within {
            true => Kind::Chapter,
            false => Kind::Book,
        };
    }
    if found.ids.iter().any(|id| id.kind == "isbn") {
        return Kind::Book;
    }
    if VOLUME.is_match(text) && found.container.is_some() {
        return Kind::Article;
    }
    // A container that is not a publisher, with a run of pages inside it, is
    // a journal however sparely the style wrote the volume out.
    let ranged = found
        .pages
        .as_deref()
        .is_some_and(|pages| pages.contains(['-', '–', '—']));
    if ranged && found.container.is_some() {
        return Kind::Article;
    }
    if found.ids.iter().any(|id| id.kind == "url") {
        return Kind::Web;
    }
    Kind::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Reference {
        one(text, None).expect("a reference")
    }

    #[test]
    fn a_book_in_a_bibliography() {
        let found =
            read("Smith, John. *The Making of the Second Estate*. Oxford: Clarendon Press, 1998.");
        assert_eq!(found.authors, ["Smith, John"]);
        assert_eq!(found.year, Some(1998));
        assert_eq!(
            found.title.as_deref(),
            Some("The Making of the Second Estate")
        );
        assert_eq!(found.container.as_deref(), Some("Clarendon Press"));
        assert_eq!(found.kind, Kind::Book);
        // The year of an imprint is not the page the work was cited at.
        assert_eq!(found.pages, None);
    }

    #[test]
    fn an_article_with_its_journal_in_italics() {
        let found = read(
            "Doe, Jane, and Peter Brown. \"Obligation and Its Limits.\" \
             *Journal of Medieval Studies* 12, no. 3 (2001): 45-67.",
        );
        assert_eq!(found.authors, ["Doe, Jane", "Peter Brown"]);
        assert_eq!(found.title.as_deref(), Some("Obligation and Its Limits"));
        assert_eq!(
            found.container.as_deref(),
            Some("Journal of Medieval Studies")
        );
        assert_eq!(found.pages.as_deref(), Some("45-67"));
        assert_eq!(found.kind, Kind::Article);
    }

    #[test]
    fn an_author_date_entry_puts_the_journal_in_the_italics_not_the_title() {
        let found = read(
            "Peirce, C. S. (1878). How to make our ideas clear. \
             *Popular Science Monthly*, 12, 286-302.",
        );
        assert_eq!(found.authors, ["Peirce, C. S."]);
        assert_eq!(found.title.as_deref(), Some("How to make our ideas clear"));
        assert_eq!(found.container.as_deref(), Some("Popular Science Monthly"));
        assert_eq!(found.pages.as_deref(), Some("286-302"));
    }

    #[test]
    fn an_author_date_entry_need_not_bracket_its_year() {
        let found = read(
            "Altmann, M. 1951, A Study of Behaviour in a Horse-Mule Group. Sociometry 14, 351-354.",
        );
        assert_eq!(found.authors, ["Altmann, M."]);
        assert_eq!(found.year, Some(1951));
        assert_eq!(
            found.title.as_deref(),
            Some("A Study of Behaviour in a Horse-Mule Group")
        );
        // The volume belongs to the journal and is not part of its name.
        assert_eq!(found.container.as_deref(), Some("Sociometry"));
    }

    #[test]
    fn a_chapter_appeared_in_the_book_and_not_in_its_publisher() {
        let found = read(
            "Brown, Peter. \"Sorcery, Demons and the Rise of Christianity.\" \
             In *Witchcraft Confessions and Accusations*, edited by Mary Douglas, \
             17-45. London: Tavistock, 1970.",
        );
        assert_eq!(found.kind, Kind::Chapter);
        assert_eq!(
            found.title.as_deref(),
            Some("Sorcery, Demons and the Rise of Christianity")
        );
        assert_eq!(
            found.container.as_deref(),
            Some("Witchcraft Confessions and Accusations")
        );
    }

    #[test]
    fn a_note_that_marks_nothing_up_is_still_read() {
        let found = read("John MacGowan, The Imperial History of China (1897), p. 154.");
        assert_eq!(found.authors, ["John MacGowan"]);
        assert_eq!(
            found.title.as_deref(),
            Some("The Imperial History of China")
        );
        assert_eq!(found.year, Some(1897));
        assert_eq!(found.pages.as_deref(), Some("154"));
    }

    #[test]
    fn what_a_note_says_before_the_citation_is_not_the_author() {
        for lead in ["Quoted in ", "See ", "See, for example, ", "Cf. "] {
            let found = read(&format!(
                "{lead}Eugene F. Hemrick, One Nation Under God (Huntington, IN: Our Sunday Visitor, 2001), 72-74."
            ));
            assert_eq!(found.authors, ["Eugene F. Hemrick"], "{lead}");
            assert_eq!(
                found.container.as_deref(),
                Some("Our Sunday Visitor"),
                "{lead}"
            );
        }
    }

    #[test]
    fn a_contributors_biography_is_not_a_citation() {
        // Opens exactly as a Chicago note does and then runs on for three
        // sentences, which a citation does not.
        let found = read(
            "Thomas L. Sheridan, SJ, is Professor Emeritus of Theology at St \
             Peter's College. He is the author of Newman on Justification (1967).",
        );
        assert!(found.authors.is_empty(), "{found:#?}");
    }

    #[test]
    fn an_editor_is_a_person_and_not_a_disqualification() {
        let found = read(
            "Eberhardt, Christian, ed. *Ritual and Metaphor: Sacrifice in the Bible*. Atlanta: Society of Biblical Literature, 2011.",
        );
        assert_eq!(found.authors, ["Eberhardt, Christian"]);
        assert_eq!(
            found.title.as_deref(),
            Some("Ritual and Metaphor: Sacrifice in the Bible")
        );
    }

    #[test]
    fn an_entry_with_nothing_but_commas_is_read_between_the_names_and_the_imprint() {
        let found = read(
            "Schopflin, George, Nations, Identity, Power, New York: New York University Press, 2000",
        );
        assert_eq!(found.authors, ["Schopflin, George"]);
        assert_eq!(found.title.as_deref(), Some("Nations, Identity, Power"));
        assert_eq!(
            found.container.as_deref(),
            Some("New York University Press")
        );
        assert_eq!(found.year, Some(2000));
    }

    #[test]
    fn a_continuation_is_flagged_and_not_resolved() {
        let found = read("Ibid., pp. 23-24, 39.");
        assert!(found.continuation);
        assert!(found.title.is_none());
        assert!(found.authors.is_empty());
        assert_eq!(found.pages.as_deref(), Some("23-24"));
        // Nothing to ask a model about either: the entry does not say.
        assert!(found.missing().is_empty());
    }

    #[test]
    fn identifiers_are_normalised_and_the_doi_is_not_repeated_as_a_url() {
        let found = read(
            "Chen, L. (2019). A study of things. Nature, 570(7761), 321-325. \
             https://doi.org/10.1038/s41586-019-1234-5",
        );
        assert_eq!(found.ids.len(), 1);
        assert_eq!(found.ids[0].kind, "doi");
        assert_eq!(found.ids[0].id, "10.1038/s41586-019-1234-5");
        assert_eq!(found.kind, Kind::Article);
    }

    #[test]
    fn a_preprint_has_an_identifier_and_no_journal() {
        let found =
            read("Vaswani, A., et al. (2017). Attention is all you need. arXiv:1706.03762.");
        assert_eq!(found.ids[0].kind, "arxiv");
        assert_eq!(found.kind, Kind::Article);
        // Naming arXiv as the journal would invent one.
        assert_eq!(found.container, None);
    }

    #[test]
    fn a_heading_or_a_page_number_is_not_a_reference() {
        for junk in ["Bibliography", "233", "Notes to pages 12-14", "see below"] {
            assert!(one(junk, None).is_none(), "{junk}");
        }
    }

    #[test]
    fn the_same_entry_reads_the_same_way_every_time() {
        let entry = "Smith, John. *A Title*. Oxford: Clarendon Press, 1998.";
        assert_eq!(one(entry, None), one(entry, None));
    }

    #[test]
    fn a_year_range_inside_a_title_is_not_a_page_range() {
        let found = read(
            "A. J. P. Taylor, The Struggle for Mastery in Europe 1848-1918 (Oxford: Oxford University Press, 1954), p. 102.",
        );
        assert_eq!(found.pages.as_deref(), Some("102"));
    }

    #[test]
    fn sentences_do_not_stop_at_an_initial_or_an_abbreviation() {
        assert_eq!(
            sentences("Tolkien, J. R. R. The Hobbit, rev. ed. London, 1951."),
            ["Tolkien, J. R. R. The Hobbit, rev. ed. London, 1951."]
        );
    }
}
