//! Cutting a block of references into the references in it.
//!
//! Two shapes arrive. A page of notes numbers its entries, and the numbering
//! says exactly where each one starts — that case is decided, not guessed at,
//! and the pattern that recognises it is the one `chunkmd` already uses to
//! recognise a page of notes, so the two cannot drift apart.
//!
//! A bibliography numbers nothing. There the signal is layout: a blank line
//! between entries, a hanging indent under each, or the surname-comma opening
//! that nearly every author-first entry begins with. Whichever of those the
//! block actually carries is the one used, and when it carries none of them
//! the fallback is one entry per line, which is right for a list that was
//! never wrapped and visibly wrong for one that was.

use std::sync::LazyLock;

use regex::Regex;

/// One entry, cut out of the block.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The entry's text, with its marker removed and whitespace collapsed.
    pub text: String,
    /// The note number it carried, where it carried one.
    pub marker: Option<String>,
}

/// How far into a block the first note number may sit before what precedes
/// it is read as something other than a note.
///
/// About the length of one full reference. Past that it is the body of the
/// page, sitting above the notes at its foot.
const HEAD_LIMIT: usize = 400;

/// At least this many numbered notes before the numbering is believed.
///
/// Two: one is a stray list item, and a real page of notes has more.
const MIN_NOTES: usize = 2;

/// The opening of an author-first bibliography entry: a surname, a comma,
/// and something after it. Deliberately narrow — it is being used to decide
/// where an entry *starts*, and a loose version would cut entries in half at
/// every "Oxford, 1998".
static SURNAME_FIRST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[\p{Lu}][\p{L}'’\-]{1,30}(?:\s+[\p{Lu}][\p{L}'’\-]{1,30}){0,2},\s+[\p{Lu}]")
        .expect("a valid pattern")
});

/// A leading note number, once the tags around it are gone.
///
/// Three digits at most, which is what `chunkmd` counts as a note number.
/// Four would swallow the year off the front of an author-date entry.
static LEADING_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[?(\d{1,3})\]?\\?[.)]?\s+").expect("a valid pattern"));

/// An HTML tag, which conversion leaves wrapped around note numbers.
static TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"</?[a-zA-Z][^>]*>").expect("a valid pattern"));

/// A markdown link left where a back-reference anchor was. The href points
/// at a fragment in some EPUBs and at another file in most: `[42](#note-42)`
/// and `[4.](part0011.html#ke_00001)` are the same marker.
static ANCHOR_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[\s*(\d{1,4})\s*\\?[.)]?\s*\]\([^)\s]*\)\s*[.)]?\s*").expect("a valid pattern")
});

/// A marker `chunkmd` does not count, because it does not need to: a
/// back-reference whose link points at another file rather than at a
/// fragment. `chunkmd` is deciding whether a chunk is a page of notes, and
/// one missed marker among thirty does not change that answer. Here a missed
/// marker runs two references together, so these are added to its.
static FILE_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]{0,8}(?:[>•]+[ \t]*|-[ \t]+)?\[\s*\d{1,3}\s*\\?[.)]?\s*\]\([^)\s]*\)")
        .expect("a valid pattern")
});

/// What citefind counts as citing something on top of what `chunkmd` does:
/// a year of publication with a number after it, which is every author-date
/// entry ever set — `Altmann, M. 1951, A Study of Behaviour. Sociometry 14,
/// 351-354.` names an author, a year, a journal and a page range, and
/// carries none of the punctuation `chunkmd` looks for.
///
/// Wider than `chunkmd`'s for the same reason the marker pattern is: it is
/// deciding whether a chunk is a page of notes, where one missed entry in
/// thirty changes nothing, and this is deciding whether a reference exists.
/// What gets through and is not a reference is refused when it is read.
static LOCAL_SIGNAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\b(?:1[4-9]|20)\d{2}\b.*?\d").expect("a valid pattern"));

/// A bullet or a blockquote marker, which conversion puts in front of a note
/// often enough to matter. A leading `*` is left alone: it opens an italic
/// title as often as it opens a list.
static BULLET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[>•]+\s*|-\s+)+").expect("a valid pattern"));

/// An id-style marker: `fn39.`, `note12)`.
static ID_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:fn|ftn|note)(\d{1,4})[.)]\s+").expect("a valid pattern"));

/// Cut a block into entries.
pub fn entries(text: &str) -> Vec<Entry> {
    let numbered = numbered_notes(text);
    let cut = match numbered.len() >= MIN_NOTES {
        true => numbered,
        false => unnumbered(text),
    };
    cut.into_iter().flat_map(divide).collect()
}

/// The least an entry can be and still be one, on either side of a split.
const MIN_WORK: usize = 25;

/// Where another citation begins inside one entry: a semicolon, and then a
/// name with a comma after it.
///
/// A semicolon and nothing else. A full stop separates works in a note too,
/// but it also separates the author from the title in every bibliography
/// entry ever written — `Kahneman, Daniel. Thinking, Fast and Slow.` would
/// come apart at the one place it must not.
static ANOTHER_WORK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#";\s+(?:(?i:see(?:\s+also)?|cf\.|and)\s+)?[\p{Lu}][\p{L}.'’\-]{1,24}(?:\s+[\p{Lu}][\p{L}.'’\-]{1,24}){0,3},\s+[\p{Lu}\*_"“‘]"#,
    )
    .expect("a valid pattern")
});

/// Cut one entry into the works it cites.
///
/// A note commonly cites two or three in a row, separated by semicolons —
/// `Adorno, *Negative Dialectics* (London, 1973), 365; Alexander, "On the
/// Social Construction of Moral Universals", 196`. They are distinct works
/// and each deserves its own record. The note number is kept on all of them,
/// because all of them are that note.
fn divide(entry: Entry) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut rest = entry.text.as_str();
    while let Some(found) = ANOTHER_WORK.find(rest) {
        // The separator belongs to neither side, and both sides have to be
        // long enough to be a citation rather than an abbreviation.
        let (before, after) = (
            &rest[..found.start()],
            rest[found.start() + 1..].trim_start(),
        );
        if before.chars().count() < MIN_WORK || after.chars().count() < MIN_WORK {
            break;
        }
        out.push(before.trim().to_string());
        rest = after;
    }
    if out.is_empty() {
        return vec![entry];
    }
    out.push(rest.trim().to_string());
    out.into_iter()
        .filter(|text| !text.is_empty())
        .map(|text| Entry {
            text,
            marker: entry.marker.clone(),
        })
        .collect()
}

/// Entries cut at their note numbers.
///
/// An entry runs from its marker to the next one. Only entries that cite
/// something are kept: a page of notes mixes bibliographic notes with
/// discursive ones, and a remark about the argument is not a reference.
fn numbered_notes(text: &str) -> Vec<Entry> {
    let mut starts = chunkmd::footnote::note_starts(text);
    starts.extend(FILE_ANCHOR.find_iter(text).map(|found| found.start()));
    starts.sort_unstable();
    starts.dedup();
    // Decided before the head is added, so that one stray `12.` in a
    // bibliography cannot turn the whole of it into two entries.
    if starts.len() < MIN_NOTES {
        return Vec::new();
    }
    // Whatever is in front of the first marker is the first note, when the
    // marker it carried was one neither pattern knows — but only when it is
    // about the length of a reference. A page of body text with its notes
    // at the foot has a marker a long way in, and everything above it is the
    // page, not a note that lost its number.
    if (1..=HEAD_LIMIT).contains(&starts[0]) {
        starts.insert(0, 0);
    }

    let mut out = Vec::new();
    for (at, start) in starts.iter().enumerate() {
        let end = starts.get(at + 1).copied().unwrap_or(text.len());
        let raw = &text[*start..end];
        if !chunkmd::footnote::cites_something(raw) && !LOCAL_SIGNAL.is_match(raw) {
            continue;
        }
        out.push(strip_marker(raw));
    }
    out
}

/// Entries cut at whatever the layout offers.
fn unnumbered(text: &str) -> Vec<Entry> {
    let blocks: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .collect();
    if blocks.len() >= 2 {
        return blocks.iter().map(|block| strip_marker(block)).collect();
    }

    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.len() < 2 {
        return lines.iter().map(|line| strip_marker(line)).collect();
    }

    // A surname-comma opening on most lines means the list is one entry per
    // line and any line without it is a wrap of the line above. Without that,
    // fall back to the hanging indent, and without that to the lines
    // themselves.
    let openers = lines
        .iter()
        .filter(|line| SURNAME_FIRST.is_match(line.trim_start()))
        .count();
    let by_surname = openers >= 2 && openers * 2 >= lines.len();

    let mut out: Vec<String> = Vec::new();
    for line in &lines {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let starts_entry = match by_surname {
            true => SURNAME_FIRST.is_match(line.trim_start()),
            false => !indented,
        };
        match (starts_entry, out.last_mut()) {
            (false, Some(previous)) => {
                previous.push(' ');
                previous.push_str(line.trim());
            }
            _ => out.push(line.trim().to_string()),
        }
    }
    out.iter().map(|entry| strip_marker(entry)).collect()
}

/// Take the note number off the front of an entry, and tidy what is left.
///
/// The number arrives wrapped in whatever the converter left behind, so the
/// patterns are matched against the opening with its tags removed — and the
/// cut is then made in the original, so that an italic span marking a title
/// further along survives.
fn strip_marker(raw: &str) -> Entry {
    let text = tidy(raw);
    let text = BULLET.replace(&text, "").to_string();
    let (bare, from) = visible(&text, 160);

    for pattern in [&*ANCHOR_LINK, &*ID_MARKER, &*LEADING_NUMBER] {
        let Some(found) = pattern.captures(&bare) else {
            continue;
        };
        let whole = found.get(0).expect("the whole match");
        let consumed = bare[..whole.end()].chars().count();
        let cut = from[consumed];
        return Entry {
            // `[4] . Smith, John` leaves the stop that belonged to the
            // marker sitting in front of the entry.
            text: text[cut..]
                .trim_start_matches(|c: char| ".) ".contains(c))
                .to_string(),
            marker: Some(found[1].to_string()),
        };
    }

    if let Some((marker, consumed)) = superscript(&bare) {
        return Entry {
            text: text[from[consumed]..].trim_start().to_string(),
            marker: Some(marker),
        };
    }

    Entry { text, marker: None }
}

/// The opening of an entry with its tags dropped, and where each character
/// that was kept came from in the original.
///
/// The last element is where the kept text ends, so that a marker running to
/// the end of the opening still has somewhere to cut.
fn visible(text: &str, limit: usize) -> (String, Vec<usize>) {
    let mut out = String::new();
    let mut from = Vec::new();
    let mut at = 0;
    while at < text.len() && from.len() < limit {
        let rest = &text[at..];
        if rest.starts_with('<') {
            if let Some(close) = rest.find('>') {
                if TAG.is_match(&rest[..=close]) {
                    at += close + 1;
                    continue;
                }
            }
        }
        let ch = rest.chars().next().expect("a character");
        if !(out.is_empty() && ch.is_whitespace()) {
            out.push(ch);
            from.push(at);
        }
        at += ch.len_utf8();
    }
    from.push(at);
    (out, from)
}

/// A leading run of superscript digits, as the number it spells and how many
/// characters it took.
fn superscript(text: &str) -> Option<(String, usize)> {
    const SUPER: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    let mut digits = String::new();
    let mut taken = 0;
    for ch in text.chars() {
        match SUPER.iter().position(|s| *s == ch) {
            Some(value) => {
                digits.push(char::from_digit(value as u32, 10)?);
                taken += 1;
            }
            None => break,
        }
    }
    if digits.is_empty() {
        return None;
    }
    // Step over the space the marker is separated from its entry by.
    let after: String = text
        .chars()
        .skip(taken)
        .take_while(|c| c.is_whitespace())
        .collect();
    Some((digits, taken + after.chars().count()))
}

/// Collapse the whitespace a converted entry is wrapped with.
fn tidy(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.text.as_str()).collect()
    }

    #[test]
    fn a_page_of_numbered_notes_is_cut_at_its_numbers() {
        let page = "\
1. John Smith, *The Title* (Oxford: Clarendon Press, 1998), 183.
2. Jane Doe, *Another Book* (Cambridge: University Press, 2001), p. 12.";
        let found = entries(page);
        assert_eq!(found.len(), 2, "{found:#?}");
        assert_eq!(found[0].marker.as_deref(), Some("1"));
        assert_eq!(found[1].marker.as_deref(), Some("2"));
        assert!(found[0].text.starts_with("John Smith"), "{:?}", found[0]);
    }

    #[test]
    fn a_note_that_cites_nothing_is_not_a_reference() {
        let page = "\
1. John Smith, *The Title* (Oxford: Clarendon Press, 1998), 183.
2. I am indebted to the seminar for this point.
3. Jane Doe, *Another Book* (Cambridge: University Press, 2001), p. 12.";
        let found = entries(page);
        assert_eq!(
            found
                .iter()
                .filter_map(|e| e.marker.clone())
                .collect::<Vec<_>>(),
            ["1", "3"]
        );
    }

    #[test]
    fn an_author_date_note_carries_none_of_the_punctuation_chunkmd_looks_for() {
        // Author, year, journal, volume, pages — and not one bracketed year,
        // `p.`, `ed.` or `Press,` among them.
        let page = "\
1. Altmann, M. 1951, A Study of Behaviour in a Horse-Mule Group. Sociometry 14, 351-354.
2. Huxley, J. S. 1957: Evolution in Action. London.";
        let found = entries(page);
        assert_eq!(found.len(), 2, "{found:#?}");
        assert!(found[0].text.starts_with("Altmann"), "{:?}", found[0]);
    }

    #[test]
    fn blank_lines_cut_a_bibliography() {
        let block = "Smith, John. The Title. Oxford: Clarendon, 1998.\n\nDoe, Jane. Another.\nCambridge, 2001.";
        let found = entries(block);
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].text, "Doe, Jane. Another. Cambridge, 2001.");
    }

    #[test]
    fn a_surname_opening_cuts_a_list_that_was_wrapped() {
        let block = "\
Smith, John. The Title of the Book That Ran On.
Oxford: Clarendon Press, 1998.
Doe, Jane. Another Work Entirely. Cambridge, 2001.";
        let found = entries(block);
        assert_eq!(
            texts(&found),
            [
                "Smith, John. The Title of the Book That Ran On. Oxford: Clarendon Press, 1998.",
                "Doe, Jane. Another Work Entirely. Cambridge, 2001.",
            ]
        );
    }

    #[test]
    fn a_hanging_indent_cuts_a_list_with_no_surnames_to_go_on() {
        let block = "\
The Oxford Companion to Everything. Oxford, 1998.
    Second line of the same entry.
Another Anonymous Work. Cambridge, 2001.";
        let found = entries(block);
        assert_eq!(found.len(), 2, "{found:#?}");
        assert!(found[0].text.ends_with("same entry."), "{:?}", found[0]);
    }

    #[test]
    fn a_marker_wrapped_in_tags_is_still_a_marker() {
        let entry = strip_marker("<a href=\"#ref12\"><sup>12</sup></a>. Smith, *Title*, p. 4.");
        assert_eq!(entry.marker.as_deref(), Some("12"));
        assert_eq!(entry.text, "Smith, *Title*, p. 4.");
    }

    #[test]
    fn an_anchor_link_marker_keeps_the_entry_and_not_the_link() {
        let entry = strip_marker("[42](#note-42). Smith, *Title*, p. 4.");
        assert_eq!(entry.marker.as_deref(), Some("42"));
        assert_eq!(entry.text, "Smith, *Title*, p. 4.");
    }

    #[test]
    fn an_id_style_marker_is_read_as_its_number() {
        let entry = strip_marker("fn39. Smith, *Title*, p. 4.");
        assert_eq!(entry.marker.as_deref(), Some("39"));
        assert_eq!(entry.text, "Smith, *Title*, p. 4.");
    }

    #[test]
    fn a_superscript_marker_is_read_as_its_number() {
        let entry = strip_marker("¹² Smith, *Title*, p. 4.");
        assert_eq!(entry.marker.as_deref(), Some("12"));
        assert_eq!(entry.text, "Smith, *Title*, p. 4.");
    }

    #[test]
    fn a_stop_that_belonged_to_the_marker_does_not_start_the_entry() {
        let entry = strip_marker("[4] . Smith, John. *A Real Book*. Oxford, 1998.");
        assert_eq!(entry.marker.as_deref(), Some("4"));
        assert_eq!(entry.text, "Smith, John. *A Real Book*. Oxford, 1998.");
    }

    #[test]
    fn an_entry_with_no_marker_keeps_all_of_itself() {
        let entry = strip_marker("Smith, John. The Title. Oxford, 1998.");
        assert_eq!(entry.marker, None);
        assert_eq!(entry.text, "Smith, John. The Title. Oxford, 1998.");
    }

    #[test]
    fn a_year_in_a_place_of_publication_does_not_start_an_entry() {
        // "Oxford, 1998." would match a loose surname-comma rule.
        let block = "\
Smith, John. A Title.
Oxford, 1998.
Doe, Jane. Another.
Cambridge, 2001.";
        let found = entries(block);
        assert_eq!(found.len(), 2, "{found:#?}");
    }
}
