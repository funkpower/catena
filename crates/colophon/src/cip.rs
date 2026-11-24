//! The cataloguing-in-publication block, which is a catalogue record the
//! publisher printed inside the book.
//!
//! Since the 1970s most English-language publishers have printed the record
//! the Library of Congress made from the manuscript on the copyright page,
//! and since 2013 they have printed it in labelled fields. Either way it
//! names the author as a library files him, the title with its subtitle, the
//! ISBN, and the subject headings the book was classified under — which is
//! the one place in a book where its subjects are stated rather than
//! inferred.
//!
//! It is a record, so it is read as one. Nothing here guesses.

use std::sync::LazyLock;

use regex::Regex;

/// What the block said.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Cip {
    /// Title, without its subtitle.
    pub title: Option<String>,
    /// Subtitle, where the title statement had one.
    pub subtitle: Option<String>,
    /// Authors, in reading order rather than filing order.
    pub authors: Vec<String>,
    /// Editors, where the work is an edited one.
    pub editors: Vec<String>,
    /// ISBNs printed inside the block.
    pub isbns: Vec<String>,
    /// Subject headings the book was classified under.
    pub subjects: Vec<String>,
}

impl Cip {
    fn is_empty(&self) -> bool {
        self.title.is_none() && self.authors.is_empty() && self.subjects.is_empty()
    }
}

static HEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)catalog(?:u)?ing[-\s]in[-\s]publication(?:\s+data)?").expect("a valid pattern")
});
/// `B105.C477D46 1991` / `126—dc20` — the call number, which ends the block.
static CALL_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^([A-Z]{1,3}\d{1,4}(\.\w+)*\s+\d{4}|\d{3}(\.\d+)?\s*[—–-]{1,2}\s*dc\d{2})")
        .expect("a valid pattern")
});
/// A name as a library files it: `Howard, Michael, 1922- author.`
static FILED_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<last>[A-Z][\p{L}'’\-]+(?:\s+[A-Z][\p{L}'’\-]+)?),\s+(?P<first>[A-Z][\p{L}'’\-\.]*(?:\s+[A-Z][\p{L}'’\-\.]*){0,3})\
(?:,\s*(?:\d{4}\s*[-–]\s*\d{0,4}|b\.\s*\d{4}|d\.\s*\d{4}))?\
(?:,?\s*(?:author|editor|translator|compiler)s?\.?)?\.?$",
    )
    .expect("a valid pattern")
});
/// `[Sur la traduction. English]` — the uniform title of a translation,
/// printed above the title it was translated into.
static UNIFORM_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\\?\[.*\\?\]\.?$").expect("a valid pattern"));
static LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(names|title|other titles|description|identifiers|subjects|classification|uniform title)\s*:\s*(.*)$")
        .expect("a valid pattern")
});
/// `1. Consciousness. 2. Philosophy of mind. I. Title.`
static NUMBERED_SUBJECTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*1\.\s+\S").expect("a valid pattern"));
static SUBJECT_SPLIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\b\d{1,2}\.\s+").expect("a valid pattern"));
/// The added entries a subject list ends with: `I. Title. II. Series.`
static ADDED_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+[IVX]+\.\s+.*$").expect("a valid pattern"));
static ISBN_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)isbn[:\s]*([0-9][0-9\s\-–]{8,20}[0-9Xx])").expect("a valid pattern")
});
/// The responsibility statement's separator, and what OCR makes of it.
///
/// A slash between the title and who wrote it is a solidus in a serif face,
/// and page scanners read it as a capital I or a lowercase l about as often
/// as they read it correctly.
static RESPONSIBILITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+[/Il\\|]\s+").expect("a valid pattern"));
static EDITION_TAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*[.,]?\s*[—–-]{1,2}\s*(\d+(st|nd|rd|th)|first|second|third|new|rev\.?|revised)\b.*$")
        .expect("a valid pattern")
});

fn tidy(s: &str) -> String {
    s.trim()
        .trim_matches(|c: char| c == '\\' || c == '"')
        .trim()
        .trim_end_matches(['.', ',', ';', ':', '/'])
        .trim()
        .to_string()
}

/// Where a statement of responsibility stops naming people: the physical
/// description ("p. cm.") and the identifiers a cataloguing block runs on to.
static STATEMENT_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:p\.\s?cm\b|pages\s+cm\b|cm\.|ISSN\b|ISBN\b)").expect("a valid pattern")
});

/// A role written before the names it applies to, and whose names they are.
const LEADING_ROLES: &[(&str, Role)] = &[
    ("edited and translated by ", Role::Editor),
    ("edited with an introduction by ", Role::Editor),
    ("edited by ", Role::Editor),
    ("selected and edited by ", Role::Editor),
    ("compiled by ", Role::Editor),
    ("general editor ", Role::Editor),
    ("translated by ", Role::Skip),
    ("translated from ", Role::Skip),
    ("translated with ", Role::Skip),
    ("with ", Role::Skip),
    ("foreword by ", Role::Skip),
    ("introduction by ", Role::Skip),
    ("illustrated by ", Role::Skip),
    ("notes by ", Role::Skip),
    ("by ", Role::Author),
];

/// A role written after a name: "Irving B. Weiner, editor-in-chief".
const TRAILING_ROLES: &[&str] = &[
    "editor-in-chief",
    "general editor",
    "editors",
    "editor",
    "eds.",
    "eds",
    "ed.",
];

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Author,
    Editor,
    Skip,
}

/// Split a statement of responsibility into who wrote and who edited.
///
/// Every name is held to the shape of a person's name, so the role words,
/// fragments and page counts a cataloguing block is full of ("edited by" on
/// a line of its own, "translated from the French", "p. cm") are dropped
/// rather than read as people.
fn people(statement: &str) -> (Vec<String>, Vec<String>) {
    let statement = match STATEMENT_END.find(statement) {
        Some(found) => &statement[..found.start()],
        None => statement,
    };
    let mut authors = Vec::new();
    let mut editors = Vec::new();
    for part in statement.split(';') {
        // "[edited by] Konrad H. Kinzl": the role in the brackets a
        // cataloguer adds when the book did not state it.
        let part = part.trim().replacen('[', "", 1).replacen(']', "", 1);
        let part = part.trim();
        let lower = part.to_lowercase();
        let (mut role, mut rest) = (Role::Author, part);
        for (prefix, given) in LEADING_ROLES {
            if lower.starts_with(prefix) {
                role = *given;
                rest = &part[prefix.len()..];
                break;
            }
        }
        if role == Role::Skip {
            continue;
        }
        // "Alan Carr.— 2nd ed": the edition statement that follows the names.
        let rest = EDITION_TAIL.replace(rest, "");
        for name in rest.split(" and ").flat_map(|n| n.split(',')) {
            let mut name = tidy(name);
            let mut this_role = role;
            let lowered = name.to_lowercase();
            if let Some(suffix) = TRAILING_ROLES.iter().find(|r| lowered.ends_with(*r)) {
                name = tidy(&name[..name.len() - suffix.len()]);
                this_role = Role::Editor;
            }
            // A role alone on its part names whoever came before it.
            if name.is_empty() && this_role == Role::Editor {
                if let Some(last) = authors.pop() {
                    editors.push(last);
                }
                continue;
            }
            if name.split_whitespace().count() > 5 {
                continue;
            }
            let Some(person) = bibclean::author::clean(&name) else {
                continue;
            };
            if !reads_as_a_name(&person) {
                continue;
            }
            if person.split_whitespace().count() < 2 {
                continue;
            }
            match this_role {
                Role::Editor => editors.push(person),
                _ => authors.push(person),
            }
        }
    }
    (authors, editors)
}

/// Whether a candidate reads as a name rather than a line of a title: no
/// little words ("Paths TO"), and no word left in capitals beside words that
/// are not ("WISDOM’S Journey").
pub(crate) fn reads_as_a_name(name: &str) -> bool {
    const LITTLE: &[&str] = &[
        "a", "an", "and", "for", "in", "of", "on", "the", "to", "with",
    ];
    let words: Vec<&str> = name.split_whitespace().collect();
    if words
        .iter()
        .any(|w| LITTLE.contains(&w.to_lowercase().as_str()))
    {
        return false;
    }
    let shouting = |w: &&str| {
        let letters: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
        letters.len() > 3 && letters.iter().all(|c| c.is_uppercase())
    };
    !(words.iter().any(shouting) && !words.iter().all(shouting))
}

fn filed_to_reading(line: &str) -> Option<String> {
    let caps = FILED_NAME.captures(line)?;
    Some(format!(
        "{} {}",
        tidy(caps.name("first")?.as_str()),
        tidy(caps.name("last")?.as_str())
    ))
}

fn split_title(statement: &str) -> (Option<String>, Option<String>, Vec<String>, Vec<String>) {
    let mut pieces = RESPONSIBILITY.splitn(statement, 2);
    let title_part = pieces.next().unwrap_or_default();
    let (authors, editors) = pieces.next().map(people).unwrap_or_default();
    let title_part = EDITION_TAIL.replace(title_part, "");
    let mut halves = title_part.splitn(2, " : ");
    let title = tidy(halves.next().unwrap_or_default());
    let subtitle = halves.next().map(tidy).filter(|s| !s.is_empty());
    (
        Some(title).filter(|t| t.len() >= 2),
        subtitle,
        authors,
        editors,
    )
}

fn subjects_from(line: &str) -> Vec<String> {
    let trimmed = ADDED_ENTRY.replace(line, "");
    SUBJECT_SPLIT
        .split(&trimmed)
        .map(tidy)
        .filter(|s| s.len() >= 3 && s.split_whitespace().count() <= 12)
        .collect()
}

fn isbns_from(line: &str) -> Vec<String> {
    ISBN_LINE
        .captures_iter(line)
        .filter_map(|c| {
            let digits = bibclean::isbn::digits(&c[1]);
            bibclean::isbn::valid(&digits).then_some(digits)
        })
        .collect()
}

/// Read the CIP block out of a book's front matter, if it printed one.
pub fn read(front: &str) -> Option<Cip> {
    let start = HEAD.find(front)?.end();
    let mut block: Vec<&str> = Vec::new();
    for line in front[start..].lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            if block.len() > 2 {
                break;
            }
            continue;
        }
        if CALL_NUMBER.is_match(line) {
            break;
        }
        block.push(line);
        if block.len() >= 24 {
            break;
        }
    }
    if block.is_empty() {
        return None;
    }

    let mut cip = Cip::default();
    let labelled = block.iter().any(|l| {
        LABEL
            .captures(l)
            .is_some_and(|c| matches!(c[1].to_lowercase().as_str(), "names" | "title" | "subjects"))
    });

    if labelled {
        for line in &block {
            let Some(caps) = LABEL.captures(line) else {
                continue;
            };
            let value = caps[2].trim();
            match caps[1].to_lowercase().as_str() {
                "names" => {
                    for name in value.split('|') {
                        if let Some(read) = filed_to_reading(tidy(name).as_str()) {
                            cip.authors.push(read);
                        }
                    }
                }
                "title" => {
                    let (title, subtitle, authors, editors) = split_title(value);
                    cip.title = title;
                    cip.subtitle = subtitle;
                    if cip.authors.is_empty() {
                        cip.authors = authors;
                    }
                    cip.editors = editors;
                }
                "identifiers" => cip.isbns.extend(isbns_from(value)),
                "subjects" => {
                    let value = value.trim_start_matches("LCSH:").trim();
                    for subject in value.split('|') {
                        let subject = tidy(subject);
                        if subject.len() >= 3 {
                            cip.subjects.push(subject);
                        }
                    }
                }
                _ => {}
            }
        }
    } else {
        let mut seen_title = false;
        for line in &block {
            if UNIFORM_TITLE.is_match(line) {
                continue;
            }
            if !seen_title {
                if let Some(name) = filed_to_reading(line) {
                    cip.authors.push(name);
                    continue;
                }
                if ISBN_LINE.is_match(line) || line.to_lowercase().starts_with("isbn") {
                    cip.isbns.extend(isbns_from(line));
                    continue;
                }
                if RESPONSIBILITY.is_match(line) || line.contains(" : ") {
                    let (title, subtitle, authors, editors) = split_title(line);
                    cip.title = title;
                    cip.subtitle = subtitle;
                    if cip.authors.is_empty() {
                        cip.authors = authors;
                    }
                    cip.editors = editors;
                    seen_title = true;
                    continue;
                }
            }
            if NUMBERED_SUBJECTS.is_match(line) {
                cip.subjects = subjects_from(line);
            }
            cip.isbns.extend(isbns_from(line));
        }
    }

    cip.isbns.sort();
    cip.isbns.dedup();
    if cip.is_empty() { None } else { Some(cip) }
}

#[cfg(test)]
mod people_tests {
    use super::people;

    #[test]
    fn role_words_are_not_people() {
        for statement in [
            "edited by",
            "translated from the French",
            "selected and",
            "index",
            "ISSN 0920-",
        ] {
            assert_eq!(people(statement), (vec![], vec![]), "{statement}");
        }
    }

    #[test]
    fn the_names_are_read_and_the_roles_kept() {
        assert_eq!(
            people("Greg Palast. p. cm"),
            (vec!["Greg Palast".to_string()], vec![])
        );
        assert_eq!(
            people("Alan Carr.— 2nd ed"),
            (vec!["Alan Carr".to_string()], vec![])
        );
        assert_eq!(
            people("[edited by] Konrad H. Kinzl"),
            (vec![], vec!["Konrad H. Kinzl".to_string()])
        );
        assert_eq!(people("Paths TO"), (vec![], vec![]));
        assert_eq!(
            people("Irving B. Weiner, editor-in-chief"),
            (vec![], vec!["Irving B. Weiner".to_string()])
        );
        assert_eq!(
            people("edited by Stanley Hauerwas and Samuel Wells"),
            (
                vec![],
                vec!["Stanley Hauerwas".to_string(), "Samuel Wells".to_string()]
            )
        );
        assert_eq!(
            people("by David W. Fischer and Alan E. Bessette ; foreword by Gary Lincoff"),
            (
                vec![
                    "David W. Fischer".to_string(),
                    "Alan E. Bessette".to_string()
                ],
                vec![]
            )
        );
    }
}
