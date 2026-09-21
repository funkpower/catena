//! The title page, read as the block it is.
//!
//! A title page is laid out rather than written: the title, sometimes broken
//! across lines by nothing but the width of the page, then whoever wrote it,
//! then the publisher at the foot. Reading it is a matter of knowing where
//! the title stops, which is at the first line that is a person or a press.

use std::sync::LazyLock;

use regex::Regex;

use crate::page::PERSON;

/// What a title page printed.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TitlePage {
    /// The title, with lines the typesetter broke put back together.
    pub title: Option<String>,
    /// What followed it, where the page gave a subtitle.
    pub subtitle: Option<String>,
    /// Whoever it says wrote the book.
    pub authors: Vec<String>,
    /// Whoever it says edited it.
    pub editors: Vec<String>,
}

/// `(1714)`, `, 1922-2019` — when a title page dates its author.
static LIFESPAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\s*[(,]\s*(?:1[0-9]|20)\d{2}\s*(?:[-\u{2013}]\s*(?:(?:1[0-9]|20)\d{2})?)?\s*\)?\s*$",
    )
    .expect("a valid pattern")
});
/// `350 BC`, `1971` — a date alone on a line, which is not a title.
static BARE_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\d{1,4}\s*(bc|ad|bce|ce)?\.?$").expect("a valid pattern"));
/// `Edited by`, with whoever it was set on the line beneath.
static ROLE_ONLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(edited|translated|written|selected|compiled)\s+by:?$")
        .expect("a valid pattern")
});
static BYLINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?P<role>by|edited by|written by|translated by|selected by|compiled by)\s+(?P<who>.{3,90})$")
        .expect("a valid pattern")
});
/// A press, an imprint, or the line that announces one. The foot of the page.
pub static IMPRINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(university press|[a-z]+ press\b|publish(?:er|ers|ing)\b|publications\b|\
verlag\b|éditions\b|editions\b|books? (?:ltd|inc|group)\b|\
routledge|blackwell|wiley|springer|palgrave|macmillan|penguin|\
random house|harpercollins|w\.? ?w\.? norton|doubleday|bloomsbury|eerdmans|\
brill\b|sage\b|vintage\b|knopf|faber|crossway|zondervan|clarendon|\
foundation\b|institute\b|society\b|trust\b|distributed by|\
cover (design|art|photograph)|jacket design|thank you for buying|\
typeset|designed by|illustrated by|printed by|\
copyright|all rights reserved|printed in)\b",
    )
    .expect("a valid pattern")
});
/// A city a publisher puts on a title page, alone on its line.
pub static PLACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(oxford|cambridge|london|new york|new haven|chicago|boston|princeton|\
berlin|paris|munich|rome|madrid|grand rapids|edinburgh|dublin|toronto|sydney|\
melbourne|minneapolis|philadelphia|san francisco|los angeles|leiden|amsterdam|\
copenhagen|stockholm|moscow|beijing|tokyo|delhi|nairobi)\b[\s,·•|and]*",
    )
    .expect("a valid pattern")
});
/// Words that name a part of a book rather than the book.
static STRUCTURAL: &[&str] = &[
    "abbreviations",
    "about the author",
    "acknowledgements",
    "acknowledgments",
    "afterword",
    "appendix",
    "bibliography",
    "contents",
    "dedication",
    "epigraph",
    "epilogue",
    "foreword",
    "frontispiece",
    "glossary",
    "half title",
    "illustrations",
    "index",
    "introduction",
    "list of contributors",
    "list of illustrations",
    "maps",
    "notes",
    "postscript",
    "preface",
    "prologue",
    "table of contents",
    "title page",
    "translator's note",
];

fn is_structural(line: &str) -> bool {
    let norm: String = line
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .trim()
        .to_lowercase();
    STRUCTURAL.binary_search(&norm.as_str()).is_ok()
}

fn people(who: &str) -> Vec<String> {
    who.split(&[';', '&'][..])
        .flat_map(|part| part.split(" and "))
        .map(|name| name.trim().trim_end_matches(['.', ',']).trim().to_string())
        .filter(|name| name.len() >= 3 && name.split_whitespace().count() <= 5)
        .collect()
}

/// Words a line ends on when the typesetter broke a title across it.
///
/// `Esotericism in` / `Western Culture` is one title set on two lines, and
/// nothing but the dangling preposition says so.
const CONTINUES: &[&str] = &[
    "a", "an", "and", "at", "between", "by", "for", "from", "in", "into", "of", "on", "or", "the",
    "to", "under", "with", "without",
];

/// Endings no surname has and every abstract noun does.
///
/// Two capitalised words look exactly like a name — `Martin Cohen` and
/// `Political Philosophy` are the same shape — and on a title page the
/// second one is the top half of the title. English morphology is what
/// tells them apart without a dictionary of names to consult.
const NOT_A_NAME: &[&str] = &[
    "ance", "ence", "ics", "ism", "ity", "ment", "ness", "ography", "ology", "osophy", "sion",
    "tion", "ture",
];

fn could_be_a_person(line: &str) -> bool {
    let line = LIFESPAN.replace(line, "");
    let line = line.trim().trim_end_matches('.');
    if !PERSON.is_match(line) {
        return false;
    }
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() > 4 {
        return false;
    }
    !tokens.iter().any(|token| {
        let word = token.trim_end_matches('.').to_lowercase();
        word.len() > 4 && NOT_A_NAME.iter().any(|end| word.ends_with(end))
    })
}

fn wraps(line: &str) -> bool {
    if line.ends_with('-') {
        return true;
    }
    line.rsplit_once(char::is_whitespace)
        .is_some_and(|(_, last)| CONTINUES.contains(&last.to_lowercase().as_str()))
}

/// Put a title the page broke across lines back together.
fn unwrap_lines(block: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in block {
        match out.last_mut() {
            Some(previous) if wraps(previous) => {
                if previous.ends_with('-') {
                    previous.pop();
                    previous.push_str(line);
                } else {
                    previous.push(' ');
                    previous.push_str(line);
                }
            }
            _ => out.push((*line).to_string()),
        }
    }
    out
}

/// Read a title page, or decline to.
///
/// The page is taken as a block that ends at the imprint — a press, a place,
/// or the first line of running prose. What is left is the title, whoever
/// wrote it, and nothing else, because that is all a title page holds.
///
/// Where the byline sits is the publisher's choice and both choices are
/// common, so it is looked for at the top of the block and at the bottom and
/// nowhere between: a name in the middle of a title page is a title that
/// happens to be two capitalised words.
///
/// Declines when the block is empty or runs past five lines, at which point
/// this is a page of something else and a title taken off it would be a
/// guess with a page number attached.
pub fn read(lines: &[String]) -> Option<TitlePage> {
    let mut out = TitlePage::default();
    let mut block: Vec<&str> = Vec::new();
    let mut waiting: Option<String> = None;

    for line in lines.iter().take(14) {
        // `Edited by` on its own line, with the names set under it.
        if let Some(role) = waiting.take() {
            let who = people(line);
            if !who.is_empty() {
                if role.starts_with("edited") {
                    out.editors.extend(who);
                } else if !role.starts_with("translated") {
                    out.authors.extend(who);
                }
                continue;
            }
        }
        if is_structural(line) || BARE_DATE.is_match(line) {
            if block.is_empty() {
                continue;
            }
            break;
        }
        if let Some(caps) = BYLINE.captures(line) {
            let role = caps["role"].to_lowercase();
            let who = people(&caps["who"]);
            if who.is_empty() {
                waiting = Some(role);
                continue;
            }
            if role.starts_with("edited") {
                out.editors.extend(who);
            } else if !role.starts_with("translated") {
                out.authors.extend(who);
            }
            continue;
        }
        if ROLE_ONLY.is_match(line) {
            waiting = Some(line.to_lowercase());
            continue;
        }
        if IMPRINT.is_match(line) || PLACE.is_match(line) {
            if block.is_empty() {
                continue;
            }
            break;
        }
        block.push(line);
        if block.len() > 5 {
            return None;
        }
    }

    if block.is_empty() {
        return None;
    }

    // The byline is above the title or below it, never inside it.
    if out.authors.is_empty() && out.editors.is_empty() && block.len() > 1 {
        if could_be_a_person(block[0]) {
            out.authors = people(block[0]);
            block.remove(0);
        } else if could_be_a_person(block[block.len() - 1]) {
            out.authors = people(block[block.len() - 1]);
            block.pop();
        }
    }

    let units = unwrap_lines(&block);
    let mut title = units.first()?.clone();
    let mut subtitle = units.get(1).cloned();
    let split = title
        .split_once(" : ")
        .or_else(|| title.split_once(": "))
        .map(|(head, tail)| (head.trim().to_string(), tail.trim().to_string()));
    if let Some((head, tail)) = split {
        title = head;
        subtitle = Some(tail);
    }

    // A fragment is not a title. Two letters of one, left behind by a font
    // the extractor could not read, is worse than nothing: it is a title
    // page's authority attached to noise.
    out.title = Some(title).filter(|t| {
        t.chars().filter(|c| c.is_alphabetic()).count() >= 4
            && t.split_whitespace()
                .any(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 3)
    });
    out.subtitle = subtitle.filter(|s| s.len() >= 2);
    out.title.as_ref()?;
    Some(out)
}
