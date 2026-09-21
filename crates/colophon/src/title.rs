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
/// `Copyright © 2013 by`, `\u{a9} 2013 Richard Swinburne`, `Copyright 1971 by`.
/// `1. General Remarks`, `2   James W. McAllister` — a numbered heading.
static NUMBERED_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\d+\s*[.)]\s+\S|^\s*\d+\s{2,}\S").expect("a valid pattern"));
/// `5222 What Is This`, `827 North Fourth Street` — a number that is not a
/// year, leading a line.
static LEADING_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\d{1,5})\s+\S").expect("a valid pattern"));
/// `Page xxxvi`, `Chapter I`, `Fig. 3` — a marker, not a name.
static MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*(page|pages|p\.|pp\.|fig\.|figure|plate|chapter|ch\.|section|part)\s+[\divxlcm]",
    )
    .expect("a valid pattern")
});
/// `Sidebottom, Harry. Ancient warfare` — a bibliography entry, which front
/// matter prints in its cataloguing block and in its further reading.
static CATALOGUE_ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Z][A-Za-z'\-]+,\s+[A-Z][A-Za-z'.\-]*\.\s+\S").expect("a valid pattern")
});
/// `To Mackenzie,` — a dedication, set alone on its own page.
static DEDICATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(to|for|in memory of)\s+\S.*,\s*$").expect("a valid pattern")
});
static COPYRIGHT_NOTICE: LazyLock<Regex> = LazyLock::new(|| {
    // The year need not follow the symbol directly: "(c) Alfredo Ferrarin
    // 2004" puts the holder between them, and was read as a title.
    Regex::new(r"(?i)(\u{a9}|\(c\)|copyright)[^\n]{0,60}?(1[0-9]|20)\d{2}")
        .expect("a valid pattern")
});
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
        r"(?i)^(?P<city>oxford|cambridge|london|new york|new haven|chicago|boston|princeton|\
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
    "abstract",
    "acknowledgements",
    "acknowledgments",
    "afterword",
    "appendix",
    "author",
    "authors",
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
    "keywords",
    "list of contributors",
    "list of illustrations",
    "maps",
    "notes",
    "postscript",
    "preface",
    "prologue",
    "subtitle",
    "table of contents",
    "title",
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
    let norm = strip_folio(&norm);
    STRUCTURAL.binary_search(&norm).is_ok()
}

/// A heading with its page number taken off either end.
///
/// Front matter is folioed in roman, and the folio sits on the same line as
/// the heading often enough to matter: `vi   CONTENTS` and `viii CONTENTS`
/// are contents pages, and were being read as titles because they are not the
/// bare word.
fn strip_folio(norm: &str) -> &str {
    let is_folio = |w: &str| {
        !w.is_empty()
            && (w.chars().all(|c| c.is_ascii_digit())
                || w.chars()
                    .all(|c| matches!(c, 'i' | 'v' | 'x' | 'l' | 'c' | 'd' | 'm')))
    };
    let mut out = norm.trim();
    loop {
        let trimmed = match out.split_once(' ') {
            Some((head, rest)) if is_folio(head) => rest.trim_start(),
            _ => match out.rsplit_once(' ') {
                Some((rest, tail)) if is_folio(tail) => rest.trim_end(),
                _ => return out,
            },
        };
        if trimmed == out {
            return out;
        }
        out = trimmed;
    }
}

/// Lines that are printed matter rather than anything a book is called.
///
/// Every one of these was read as a title off a real book in a real library:
/// a copyright notice, a translator's credit, a part heading in brackets, a
/// span of HTML a converter left behind, and a volume entry from a collected
/// edition's own list of volumes.
fn is_furniture(line: &str) -> bool {
    let t = line.trim();
    if t.starts_with('[') || t.starts_with('\\') {
        return true;
    }
    // A converter that left its markup in. Nothing printed on a title page is
    // a tag, a stylesheet declaration, a markdown link or a filename, and a
    // title that appears to be one is a fragment of the conversion rather than
    // of the book. Rejecting only the first such line is no use: the next line
    // of a mangled page is mangled too, so the whole shape has to go.
    if t.contains("<span")
        || t.contains("<div")
        || t.contains("<p ")
        || t.contains("</")
        || t.contains("](")          // a markdown link the converter split
        || t.contains("&quot;")
        || t.contains("&amp;")
        || t.contains("mso-")        // Word's stylesheet, via HTML export
        || t.contains("font-family")
        || t.contains("font-size")
        || t.contains("style=")
        || t.contains("_epub_")
        || t.contains(".html")
        || t.contains(".xhtml")
    {
        return true;
    }
    // Sentences a copyright page sets, which are not notices and so not caught
    // by the notice pattern.
    if NUMBERED_HEADING.is_match(t) || MARKER.is_match(t) || CATALOGUE_ENTRY.is_match(t) {
        return true;
    }
    // A line opening with a number that is not a year. "5222 What Is This" and
    // "827 North Fourth Street" are a stray folio and an address; "1984
    // Revisited" is a book, and the difference is whether the number could be
    // the year of one.
    if let Some(c) = LEADING_NUMBER.captures(t) {
        let n: u32 = c[1].parse().unwrap_or(0);
        if !(1000..=2099).contains(&n) {
            return true;
        }
    }
    if DEDICATION.is_match(t) {
        return true;
    }
    let lower = t.to_lowercase();
    for phrase in [
        "moral rights of the author",
        "no part of this publication",
        "british library cataloguing",
        "library of congress cataloging",
        "printed in the united",
        "typeset by",
        "a cip catalogue record",
    ] {
        if lower.contains(phrase) {
            return true;
        }
    }
    let low = t.to_lowercase();
    // The notice, not the word. "Copyright and Its Discontents" is a book;
    // "Copyright © 2013 by" is the page it would be printed on. What tells
    // them apart is the symbol or the year, never the word alone.
    if COPYRIGHT_NOTICE.is_match(t)
        || low.contains("all rights reserved")
        || low.starts_with("first published")
        || low.starts_with("reprinted")
    {
        return true;
    }
    // "Translated from the French by Willard R. Trask" — a credit, not a name
    // and not a title. ROLE_ONLY catches the bare "Translated by"; this is the
    // whole sentence, which a title page sets on one line.
    if low.starts_with("translated from") || low.starts_with("translated by") {
        return true;
    }
    // "Vol. 11. HENRY JAMES, JR." — an entry in a collected edition's list of
    // its own volumes, printed in the front matter of every volume.
    if low.starts_with("vol.") || low.starts_with("volume ") && low.contains('.') {
        return true;
    }
    false
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
        if is_structural(line) || is_furniture(line) || BARE_DATE.is_match(line) {
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

#[cfg(test)]
mod furniture_tests {
    use super::*;

    /// Every string here was read as a title off a book in a real library.
    #[test]
    fn printed_matter_is_not_a_title() {
        for line in [
            "vi   CONTENTS",
            "viii   CONTENTS",
            "xiv  ACKNOWLEDGEMENTS",
            "12 INDEX",
            "Translated from the French by Willard R. Trask",
            "Copyright © 2013 by Barry Davies, BEM",
            "© Alfredo Ferrarin 2004",
            "\u{a9} Cambridge University Press 2011",
            "[PART ONE:",
            "Piper Verlag GmbH, Munich<span style=\"font-size\">",
            "Vol. 11. HENRY JAMES, JR.",
            "All rights reserved",
            "First published 1971",
        ] {
            assert!(
                is_structural(line) || is_furniture(line),
                "read as a title: {line:?}"
            );
        }
    }

    #[test]
    fn a_real_title_survives_all_of_that() {
        for line in [
            "The First World War",
            "A Very Short Introduction",
            "Christianity and Classical Culture",
            "The Problems of Philosophy",
            "Volume and Void",
            "Contents of Consciousness",
            "August 1914",
            "Copyright and Its Discontents",
        ] {
            assert!(
                !(is_structural(line) || is_furniture(line)),
                "rejected a real title: {line:?}"
            );
        }
    }

    #[test]
    fn a_folio_comes_off_either_end() {
        assert_eq!(strip_folio("vi contents"), "contents");
        assert_eq!(strip_folio("contents vi"), "contents");
        assert_eq!(strip_folio("contents"), "contents");
        // A title that happens to be roman numerals throughout is not a folio
        // with nothing left; it keeps whatever it had.
        assert_eq!(strip_folio("i"), "i");
    }
}

#[cfg(test)]
mod conversion_tests {
    use super::*;

    /// A mangled page is mangled on every line, so rejecting the first
    /// leftover only moves the misread down one. Each of these is the line
    /// colophon fell through to after the line above it was rejected.
    #[test]
    fn what_a_converter_left_behind_is_never_a_title() {
        for line in [
            "[PART ONE:",
            "RULERS AND REDNECKS](Webb_9780767922951_epub_p01_r1.html)",
            "[AFRICAN SLAVERY IN AMERICA - \\[1775\\]](index_split_004.html)",
            "Piper Verlag GmbH, Munich<span style=\"font-family:&quot;Times\"",
            "mso-bidi-font-family:&quot;Times New Roman&quot;;mso-ansi-language",
            "The moral rights of the author have been asserted",
            "No part of this publication may be reproduced",
            "British Library Cataloguing in Publication Data",
            "Typeset by SPI Publisher Services",
        ] {
            assert!(is_furniture(line), "read as a title: {line:?}");
        }
    }

    #[test]
    fn a_title_that_merely_sounds_like_the_copyright_page_survives() {
        for line in [
            "Copyright and Its Discontents",
            "The Moral Rights of Man",
            "Printed Matter",
            "A Publication of the Institute",
        ] {
            assert!(!is_furniture(line), "rejected a real title: {line:?}");
        }
    }
}

#[cfg(test)]
mod review_pile_tests {
    use super::*;

    /// Everything here was proposed as a title by a real run over this
    /// library, and sat in the review queue waiting for a person to say no.
    #[test]
    fn what_the_review_pile_was_full_of() {
        for line in [
            "1. General Remarks",
            "2   James W. McAllister",
            "5222 What Is This",
            "827 North Fourth Street",
            "Page xxxvi",
            "Page xxxviii",
            "Chapter I",
            "title",
            "Sidebottom, Harry. Ancient warfare",
            "To Mackenzie,",
        ] {
            assert!(
                is_structural(line) || is_furniture(line),
                "still read as a title: {line:?}"
            );
        }
    }

    /// And none of it catches a book. The numbers are the delicate part: a
    /// year can open a title, and a folio cannot.
    #[test]
    fn a_book_is_still_a_book() {
        for line in [
            "1984 Revisited",
            "1914: The Year the World Ended",
            "Chapter and Verse",
            "Part of the Furniture",
            "To Kill a Mockingbird",
            "For Whom the Bell Tolls",
            "The Title of This Book",
            "Pages from a Cold Island",
            "Sections of a Broken Whole",
            "In Memory of Bread",
        ] {
            assert!(
                !(is_structural(line) || is_furniture(line)),
                "rejected a real title: {line:?}"
            );
        }
    }
}
