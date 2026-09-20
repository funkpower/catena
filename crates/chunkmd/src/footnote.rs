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

/// Whether a chunk carries footnote or endnote citation material.
///
/// The structural signal first, when the source had one. Otherwise each note
/// entry runs from its marker to the next one, capped at a few hundred
/// characters, and enough of them have to look like citations.
pub fn looks_like_citations(text: &str) -> bool {
    if text.contains(SENTINEL) {
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
