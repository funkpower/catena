//! Spotting a chunk that is really a page of numbered notes.
//!
//! Footnotes and endnotes are citations in disguise: they read as body text,
//! rank as body text, and answer a query with a publisher and a page number.
//! Flagging them lets a consumer treat them as the bibliography they are
//! without moving them out of the body, where their page provenance lives.

use std::sync::LazyLock;

use fancy_regex::Regex as FancyRegex;
use textkit::pattern::{SPACE, WORD_END, WORD_START};

/// A private-use codepoint marking the start of a note's text.
///
/// EPUB3 note definitions carry a structural signal — `epub:type="footnote"`
/// and the ARIA `doc-footnote` role — that is reliable in a way the visible
/// marker never is: `[65]`, `fn39.`, a bare superscript, all of them and
/// none of them. A converter that can see that structure marks it with this
/// character, which survives conversion to markdown verbatim. When it is
/// present, nothing below has to guess.
///
/// It is stripped from the text a chunk carries, after the flag is set.
pub const SENTINEL: char = '\u{e000}';

/// How far past its marker a note's citation signal may sit, in characters.
///
/// Citation signals sit near the front of an entry. The cap stops a marker
/// followed by pages of prose from matching a signal far away. HTML class
/// soup inflates entries, so it is generous.
const ENTRY_WINDOW: usize = 500;

/// At least this many numbered-note lines have to carry a citation signal.
///
/// Two, because one avoids a false positive from a stray "1. " list item,
/// and a real page of notes almost always has more.
const MIN_CITED_NOTES: usize = 2;

/// The start of a numbered note, in any of the shapes conversion leaves
/// behind. A marker only counts at a line start or after a run of two or
/// more spaces — several notes can share one physical line — so that a
/// superscript *reference* to a note, mid-sentence, does not qualify.
static NOTE_MARKER: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(&format!(
        r"(?xm)
    (?: ^[ \t]{{0,8}} (?:-[ \t]+)?      # line start, optional '- ' list bullet
      | (?<={SPACE}{SPACE})            # or after a 2+ space run
      | (?<=\ -\ )                    # or after ' - ' (inline list separator)
    )
    (?:
        \d{{1,3}} \\?[.)] [ \t]+ \S               # plain '12. ' or escaped '12\. '
      | \[ \d{{1,3}} \] [ \t]* \S                 # bracket-numbered '[12] Author'
      | \d{{1,3}} [ \t]+ \S                       # bare '12 ' from a page bottom
      | [¹²³⁰-⁹]+ {SPACE}* \S                     # unicode superscript marker
      | (?:fn|ftn|note) \d{{1,4}} [.)] [ \t]+ \S  # id-style 'fn39. '
      | (?:<span[^>]*>{SPACE}*)*                  # outer span wrap
        <a{WORD_END}[^>]*> {SPACE}* (?:<span[^>]*>{SPACE}*)* (?:<sup>{SPACE}*)?
            \d{{1,3}} {SPACE}* [.)]?
        {SPACE}* (?:</sup>{SPACE}*)? (?:</span>{SPACE}*)* </a>
        (?:{SPACE}*</span>)* {SPACE}* [.)]?        # anchor forms
      | \[ {SPACE}* (?:<span[^>]*>)? {SPACE}* \d{{1,3}}
        {SPACE}* (?:</span>)? {SPACE}* \] \( \#[^)]* \)  # [42](#...)
      | <sup> {SPACE}* \d{{1,3}} {SPACE}* </sup>   # bare sup note
      | \\?\[ {SPACE}* (?: \[ {SPACE}* )?      # `\[←788 \]` and
        [\u{{2190}}\u{{2191}}\u{{21D1}}\u{{23CE}}]  # `\[[←193](file#frag) \]`
        {SPACE}* \d{{1,4}} {SPACE}*
        (?: \] \( [^)\s]* \) )? {SPACE}* \\?\]
    )"
    ))
    .expect("a valid pattern")
});

/// What makes a note bibliographic rather than discursive.
/// What a citation looks like inside a note entry.
///
/// The year alternative wants Python's `\b`, which is true when exactly one
/// side is a word character. What follows it here is a digit, so the half
/// that applies is the one asserting the character *before* is not a word
/// character. `WORD_END` would ask the opposite and refuse `(1740-1811)`,
/// where the year is preceded by a dash and a date.
static CITE_SIGNAL: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(&format!(
        r"(?ix)
    \((?:[^()]*{WORD_START})?(?:1[5-9]|20)\d{{2}}\)  # parenthesised year: (Oxford, 1998)
  | {WORD_START}pp?\.{SPACE}*\d                      # p. 12 / pp. 12-34
  | {WORD_START}vol\.{SPACE}*\d                      # vol. 2
  | {WORD_START}Ibid{WORD_END}
  | {WORD_START}op\.{SPACE}*cit{WORD_END}
  | {WORD_START}loc\.{SPACE}*cit{WORD_END}
  | {WORD_START}et{SPACE}+al{WORD_END}
  | {WORD_START}(?:University|Clarendon|Academic){SPACE}+Press{WORD_END}
  | {WORD_START}Press,{SPACE}
  | {WORD_START}eds?\.{SPACE} | {WORD_START}trans\.{SPACE}
  | \*[^*\n]{{3,80}}\*{SPACE}*,?{SPACE}*\d         # short form: *Title*, 183
  | <span[^>]*italic[^>]*>                          # HTML-italic title survives conversion
  | ,{SPACE}+\d{{1,4}}{SPACE}*[.;]                   # trailing page: '..., 183.'
  | https?://                                       # web citations (URLs, DOIs)"
    ))
    .expect("a valid pattern")
});

/// Where each numbered note begins, in reading order.
///
/// Public because splitting a page of notes into the notes on it is the same
/// knowledge as recognising one, and two copies of the pattern above would
/// drift apart.
pub fn note_starts(text: &str) -> Vec<usize> {
    NOTE_MARKER
        .find_iter(text)
        .filter_map(Result::ok)
        .map(|found| found.start())
        .collect()
}

/// Whether an entry reads as a citation rather than as a remark.
///
/// Also public: a note that cites nothing is a note a bibliography has no
/// use for, and the caller deciding that should ask the same question this
/// module asks.
pub fn cites_something(entry: &str) -> bool {
    CITE_SIGNAL.is_match(entry).unwrap_or(false)
}

/// The opening of a bibliography entry: a surname, a comma, and an initial
/// or a given name. Deliberately narrow, because it is being counted across
/// a page of body prose and a loose version would match dialogue.
static ENTRY_OPENING: LazyLock<FancyRegex> = LazyLock::new(|| {
    // A converted bibliography is often a bulleted list, and the bullet sits
    // where the surname would: `•   Skorupski, J. 1995. Ethical Explorations.`
    FancyRegex::new(concat!(
        r"(?m)^[ \t]{0,12}",
        r"(?:[\u{2022}\u{00B7}\u{25AA}\u{25CF}*+\u{2013}\u{2014}-][ \t]+)?",
        r"\p{Lu}[\p{L}'\u{2019}\-]{1,25},[ \t]+(?:\p{Lu}\.|\p{Lu}[\p{L}]{1,20}[,.]?[ \t])",
    ))
    .expect("a valid pattern")
});

/// What an unnumbered entry carries that says it is one.
///
/// `CITE_SIGNAL` is about notes and looks for `p.`, `ed.`, `Press,` and a
/// bracketed year. An author-date entry — `Block, N. 1981. Psychologism and
/// behaviorism. Philosophical Review 90:5-43.` — has none of those and is
/// still plainly a reference. A year, or a page range, or a publisher.
static ENTRY_SIGNAL: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(
        r"(?ix) \b(?:1[4-9]|20)\d{2}\b
              | \b\d{1,4}\s*[:\u{2013}\-]\s*\d{1,4}\b
              | \b(?:press|verlag|publishers?|university|editions?)\b",
    )
    .expect("a valid pattern")
});

/// How many entry openings make a run worth calling a reference list.
///
/// Three. A chapter of a reader ends with four or five "further reading"
/// lines and no heading over them; a page of prose has none.
const MIN_RUN: usize = 3;

/// A page number written as a link, which is how a converted index lists
/// where a name appears: `[85](Chapter003.html#page_85)`.
static PAGE_LINK: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(r"\[\s*\d{1,4}(?:\s*[\u{2013}\-]\s*\d{1,4})?\s*\]\([^)]*\)")
        .expect("a valid pattern")
});

/// Whether an entry says what the work is, rather than only where it is.
///
/// `Newton, Isaac, 5, 93, 141, 144` opens exactly like a bibliography entry
/// and is a line of an index: a name, and then nothing but page numbers. A
/// reference names a title and a publisher, which is several real words —
/// and does not list the same page number twice as a link.
fn names_a_work(entry: &str) -> bool {
    // Two or more page links and it is an index line, whatever else it says.
    if PAGE_LINK.find_iter(entry).filter_map(Result::ok).count() >= 2 {
        return false;
    }
    entry
        .split(|c: char| !c.is_alphabetic())
        .filter(|word| word.chars().count() >= 3)
        .count()
        >= MIN_WORDS
}

/// How many words of three letters or more an entry needs before it is
/// naming a work rather than listing pages.
const MIN_WORDS: usize = 5;

/// Whether a chunk carries a run of reference entries with nothing marking
/// them — no numbers, no heading, just the shape.
///
/// A reader or a companion ends each chapter with a short list like this,
/// indented under the last paragraph of the essay. Nothing says it is a
/// bibliography, so neither a chapter title nor a note number can find it.
pub fn looks_like_reference_run(text: &str) -> bool {
    let openings: Vec<usize> = ENTRY_OPENING
        .find_iter(text)
        .filter_map(Result::ok)
        .map(|found| found.start())
        .collect();
    if openings.len() < MIN_RUN {
        return false;
    }
    // Each one has to cite something, or a page of letters signed
    // `Yours, Charles` would qualify.
    let mut cited = 0;
    for start in &openings {
        // The entry is the rest of its own line. Taking it up to the next
        // opening instead pools several lines of an index together, and four
        // page-number lines between them carry enough words to pass for a
        // title.
        let rest = &text[*start..];
        let end = rest.find('\n').unwrap_or(rest.len());
        let entry: String = rest[..end].chars().take(ENTRY_WINDOW).collect();
        if names_a_work(&entry) && ENTRY_SIGNAL.is_match(&entry).unwrap_or(false) {
            cited += 1;
            if cited >= MIN_RUN {
                return true;
            }
        }
    }
    false
}

/// Whether a chunk carries footnote or endnote citation material.
///
/// The structural signal first, when the source had one. Otherwise each note
/// entry runs from its marker to the next one, capped at a few hundred
/// characters, and enough of them have to look like citations.
pub fn looks_like_citations(text: &str) -> bool {
    if text.contains(SENTINEL) {
        return true;
    }
    // A run of entries with nothing marking them counts too: the flag means
    // "this chunk carries citation material", and a reader's end-of-chapter
    // list carries as much of it as a page of numbered notes.
    if looks_like_reference_run(text) {
        return true;
    }

    let starts = note_starts(text);
    if starts.len() < MIN_CITED_NOTES {
        return false;
    }

    let mut cited = 0;
    for (at, start) in starts.iter().enumerate() {
        let end = starts.get(at + 1).copied().unwrap_or(text.len());
        let entry: String = text[*start..end].chars().take(ENTRY_WINDOW).collect();
        if cites_something(&entry) {
            cited += 1;
            if cited >= MIN_CITED_NOTES {
                return true;
            }
        }
    }
    false
}

/// Remove the structural marker from text on its way out.
pub fn strip_sentinel(text: &str) -> String {
    text.replace(SENTINEL, "")
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_bibliography_converted_into_a_bulleted_list_is_still_one() {
        // The bullet sits where the surname would, so every entry opening
        // was missed and the whole page went unflagged — which meant the
        // reader never saw a word of it.
        let page = "\u{2022}   Skorupski, J. 1995. 'Agent-Neutrality, Consequentialism, \
                    Utilitarianism', Utilitas, 7: 49-54.\n\
                    \u{2022}   Slote, M. 1984. 'Satisficing Consequentialism', Proceedings \
                    of the Aristotelian Society, 58: 139-63.\n\
                    \u{2022}   Smart, J. J. C. 1973. 'An Outline of a System of Utilitarian \
                    Ethics'. Cambridge: Cambridge University Press.\n";
        assert!(looks_like_reference_run(page));
        assert!(looks_like_citations(page));
    }

    #[test]
    fn a_reading_list_with_nothing_marking_it_is_citation_material() {
        // The end of a chapter in a reader: no heading, no numbers, just the
        // shape, indented under the last paragraph of the essay.
        let page = "\
In the second half of the twentieth century, dualism was widely rejected.

    Block, N. 1981. Psychologism and behaviorism. Philosophical Review 90:5-43.
    Dennett, D. C. 1987. The Intentional Stance. MIT Press.
    Geach, P. 1957. Mental Acts. Routledge and Kegan Paul.
";
        assert!(looks_like_reference_run(page));
        assert!(looks_like_citations(page));
    }

    #[test]
    fn a_page_of_an_index_opens_the_same_way_and_is_not() {
        // `Surname, Given, <page numbers>` is the shape of both. What a
        // reference has after the name is a title.
        let index = "\
Newmarch, William, 18n, 233n
Newton, Isaac, 5, 93, 141, 144, 145 & n, 166 & n, 270 & n, 285 & n, 371
Newton, John, 304n, 306n, 307n
Nightingale, Florence, 315n
";
        assert!(!looks_like_reference_run(index));

        // The same, once a converter has written the pages as links.
        let linked = "\
Wedick, James J., [85](Chapter003.html#page_85), [88](Chapter003.html#page_88)
Western New York Task Force, [27](Chapter001.html#page_27), [45](Chapter002.html#page_45)
Wright, Lawrence, [13](Intro.html#page_13), [22](Chapter001.html#page_22)
";
        assert!(!looks_like_reference_run(linked));
    }

    #[test]
    fn three_entries_are_a_run_and_two_are_not() {
        let two = "\
    Block, N. 1981. Psychologism and behaviorism. Philosophical Review 90:5-43.
    Dennett, D. C. 1987. The Intentional Stance. MIT Press.
";
        assert!(!looks_like_reference_run(two));
    }

    use super::*;

    #[test]
    fn the_structural_marker_settles_it_on_its_own() {
        assert!(looks_like_citations(&format!("{SENTINEL}See above.")));
    }

    #[test]
    fn a_year_preceded_by_a_dash_is_still_a_citation() {
        // `(1740\u{2013}1811)` and `(Niter\u{f3}i, 1860\u{2013}1937)`: the year is
        // preceded by a character that is not a word character, which is the
        // half of a word boundary that applies when what follows is a digit.
        // Both are real entries out of a converted book, where the
        // parenthesised year was the only citation signal in the chunk.
        let notes = "11\\. Tiradentes was a hero. In this painting by Ant\u{f4}nio Parreiras \
                     (Niter\u{f3}i, 1860\u{2013}1937), he is immortalized.\n\n\
                     12\\. Many peoples came from Africa. Carlos Juli\u{e3}o \
                     (1740\u{2013}1811) was a Portuguese-Italian artist.\n";
        assert!(looks_like_citations(notes));
    }

    #[test]
    fn two_numbered_notes_with_citation_signals_count() {
        let text = "12. Arendt, The Human Condition (Chicago, 1958), 44.\n\
                    13. Bloch, Feudal Society, vol. 2, p. 17.";
        assert!(looks_like_citations(text));
    }

    #[test]
    fn one_note_is_not_enough() {
        assert!(!looks_like_citations(
            "12. Arendt, The Human Condition (Chicago, 1958), 44."
        ));
    }

    #[test]
    fn a_numbered_list_without_citations_is_not_notes() {
        let text = "1. Boil the water.\n2. Add the salt.\n3. Wait.";
        assert!(!looks_like_citations(text));
    }

    #[test]
    fn a_mid_sentence_reference_is_not_a_note() {
        let text = "As shown elsewhere 12. and again 13. the argument holds \
                    (Oxford, 1998) and p. 12.";
        assert!(!looks_like_citations(text));
    }

    #[test]
    fn a_signal_far_past_its_marker_does_not_count() {
        let filler = "word ".repeat(200);
        let text = format!("12. {filler}(Oxford, 1998)\n13. {filler}p. 44");
        assert!(!looks_like_citations(&text));
    }

    #[test]
    fn anchor_wrapped_markers_are_recognised() {
        let text = "<a href=\"#ref1\"><sup>1</sup></a>. Arendt, (Chicago, 1958), 44.\n\
                    <a href=\"#ref2\"><sup>2</sup></a>. Bloch, p. 17.";
        assert!(looks_like_citations(text));
    }

    #[test]
    fn the_marker_is_removed_from_the_text_that_ships() {
        assert_eq!(strip_sentinel(&format!("{SENTINEL}note")), "note");
    }
}
