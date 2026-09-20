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

/// A note number run straight into the text with no space: `36Armaud Puig`.
/// Conversion does this when the marker was a superscript.
static GLUED_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(\d{1,3})(?:\p{Lu}|\*|“|")"#).expect("a valid pattern"));

/// The copyright line a digitised book prints at the foot of every page:
/// `Cambridge Histories Online © Cambridge University Press, 2008`. It is
/// not a reference, and it is glued to the end of whatever note the page
/// ended on.
static COPYRIGHT_FOOTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"©").expect("a valid pattern"));

/// The back-reference marker a whole class of EPUB conversions leaves:
/// `\[←788 \]`, or `\[[←193](index_split_011.xhtml#citation-193) \]` where
/// the arrow is also a link back to where the note was called.
static ARROW_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x) ^ \\?\[ \s* (?:\[\s*)?
        [\x{2190}\x{2191}\x{21D1}\x{23CE}] \s*
        (\d{1,4}) \s*
        (?: \] \( [^)\s]* \) )? \s* \\?\] \s*",
    )
    .expect("a valid pattern")
});

/// An HTML comment. Conversion leaves `<!-- -->` between list items and
/// `<!-- Page 41 -->` where a page turned; neither is part of a reference.
static COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<!--.*?-->").expect("a valid pattern"));

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
    // A bibliography is not a page of notes, however many of its lines begin
    // on a number. `81. Reprinted in Mellor (1991):104-22.` is the second
    // line of an entry whose first line ran out of room, and two of those on
    // a page of twenty-three references used to cut the page in two and
    // throw both halves away for running on.
    let surnames = text
        .lines()
        .filter(|line| SURNAME_FIRST.is_match(line.trim_start()))
        .count();
    let cut = match numbered.len() >= MIN_NOTES && numbered.len() >= surnames {
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
        r#";\s+(?:(?i:see(?:\s+also)?|cf\.|and)\s+)?[\p{Lu}][\p{L}.'’\-]{1,24}(?:\s+[\p{Lu}][\p{L}.'’\-]{1,24}){0,3},\s+[\p{Lu}\*_"“‘']"#,
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
///
/// A blank line between entries is the clearest signal there is, but it is
/// not always between entries: a bibliography often has one after the
/// running head and none at all between the references under it. So each
/// block a blank line produces is looked at again, and cut by line where its
/// lines are separate entries.
fn unnumbered(text: &str) -> Vec<Entry> {
    let blocks: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .collect();

    let mut out: Vec<String> = Vec::new();
    for block in &blocks {
        out.extend(by_line(block));
    }
    out.iter().map(|entry| strip_marker(entry)).collect()
}

/// Cut one block into the entries its lines make, or leave it whole.
///
/// Two or more lines opening on a surname and a comma means the block is a
/// list and those lines are where the entries start. Failing that, a hanging
/// indent says which lines continue the one above.
fn by_line(block: &str) -> Vec<String> {
    let lines: Vec<&str> = block
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.len() < 2 {
        return lines.iter().map(|line| line.trim().to_string()).collect();
    }

    let openers = lines
        .iter()
        .filter(|line| SURNAME_FIRST.is_match(line.trim_start()))
        .count();
    // One is enough inside a block: a block is a single entry or a list of
    // them, not a mixture, so the line that opens on a surname is where an
    // entry starts and the lines that do not are its continuation.
    let by_surname = openers >= 1;

    let mut out: Vec<String> = Vec::new();
    for line in &lines {
        let is_indented = line.starts_with(' ') || line.starts_with('\t');
        let starts_entry = match by_surname {
            true => SURNAME_FIRST.is_match(line.trim_start()),
            false => !is_indented,
        };
        match (starts_entry, out.last_mut()) {
            (false, Some(previous)) => {
                previous.push(' ');
                previous.push_str(line.trim());
            }
            _ => out.push(line.trim().to_string()),
        }
    }
    out
}

/// Take the note number off the front of an entry, and tidy what is left.
///
/// The number arrives wrapped in whatever the converter left behind, so the
/// patterns are matched against the opening with its tags removed — and the
/// cut is then made in the original, so that an italic span marking a title
/// further along survives.
fn strip_marker(raw: &str) -> Entry {
    let text = cut_footer(&tidy(raw));
    let text = tidy(&COMMENT.replace_all(&text, " "));
    let text = BULLET.replace(&text, "").to_string();
    let (bare, from) = visible(&text, 160);

    for pattern in [&*ARROW_MARKER, &*ANCHOR_LINK, &*ID_MARKER, &*LEADING_NUMBER] {
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

    if let Some(found) = GLUED_NUMBER.captures(&bare) {
        let marker = found[1].to_string();
        return Entry {
            text: text[from[marker.len()]..].to_string(),
            marker: Some(marker),
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

/// Cut the copyright line off the end of an entry.
///
/// It runs from wherever the sentence holding the `©` began, because the
/// words in front of it — the name of the digitising service — belong to it
/// and not to the note the page ended on.
fn cut_footer(text: &str) -> String {
    let Some(found) = COPYRIGHT_FOOTER.find(text) else {
        return text.to_string();
    };
    let head = &text[..found.start()];
    let cut = head.rfind(". ").map_or(0, |at| at + 1);
    text[..cut].trim().to_string()
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
    fn a_line_of_a_bibliography_that_begins_on_a_number_is_not_a_note() {
        // `81. Reprinted in Mellor (1991):104-22.` is the second line of an
        // entry whose first line ran out of room. Two of those on a page of
        // references used to cut the page into two enormous entries.
        let page = "\
Martin, C.B. and Heil, John (forthcoming) \u{201c}Rules and Powers,\u{201d} Philosophical
             Perspectives.
             Mele, Alfred R. (1987) Irrationality: An Essay on Akrasia, New York: Oxford.
             Mellor, Hugh (1974) \u{201c}In Defense of Dispositions,\u{201d} Philosophical Review 83:157-
             81. Reprinted in Mellor (1991):104-22.
             Mellor, Hugh (1991) Matters of Metaphysics, Cambridge: Cambridge University
             Press.";
        let found = entries(page);
        assert!(found.len() >= 4, "{found:#?}");
        assert!(
            found.iter().any(|e| e.text.starts_with("Mele")),
            "{found:#?}"
        );
        assert!(
            found
                .iter()
                .any(|e| e.text.starts_with("Mellor, Hugh (1991)")),
            "{found:#?}"
        );
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
    fn a_running_head_does_not_make_the_bibliography_under_it_one_entry() {
        // The blank line here is after the running head, not between
        // entries — and there is none at all between the references.
        let page = "\
Catherine H. Zuckert

Rowe, C. J. 1996. \u{201c}The Politicus.\u{201d} In Form and Argument, ed. C. Gill. Oxford.
Saunders, T. J. 1992. \u{201c}Plato\u{2019}s Later Political Thought.\u{201d} Cambridge.
Scodel, H. R. 1987. Diaeresis and Myth in Plato\u{2019}s Statesman. G\u{f6}ttingen.
Stalley, R. F. 1983. An Introduction to Plato\u{2019}s Laws. Oxford.";
        let found = entries(page);
        assert!(found.len() >= 4, "{found:#?}");
        assert!(
            found.iter().any(|e| e.text.starts_with("Saunders")),
            "{found:#?}"
        );
        assert!(
            found.iter().any(|e| e.text.starts_with("Stalley")),
            "{found:#?}"
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
    fn the_copyright_line_a_digitised_book_prints_is_cut_off() {
        let entry = strip_marker(
            "Holquist, Making War, pp. 211-12. 106 Cambridge Histories Online \
             © Cambridge University Press, 2008",
        );
        assert_eq!(entry.text, "Holquist, Making War, pp. 211-12.");
    }

    #[test]
    fn the_back_reference_marker_an_epub_conversion_leaves() {
        let plain = strip_marker("\\[←788 \\]\u{a0}  Krumbacher, Byz. Lit.-Gesch., p. 969.");
        assert_eq!(plain.marker.as_deref(), Some("788"));
        assert_eq!(plain.text, "Krumbacher, Byz. Lit.-Gesch., p. 969.");

        let linked =
            strip_marker("\\[[←193](index_split_011.xhtml#citation-193) \\]\u{a0}  Œd. Rex., 642.");
        assert_eq!(linked.marker.as_deref(), Some("193"));
        assert_eq!(linked.text, "Œd. Rex., 642.");
    }

    #[test]
    fn a_comment_conversion_left_behind_is_not_part_of_a_reference() {
        let entry = strip_marker("1. Smith, *A Title* (Oxford, 1998). <!-- -->");
        assert_eq!(entry.text, "Smith, *A Title* (Oxford, 1998).");
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
