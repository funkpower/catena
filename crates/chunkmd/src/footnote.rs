//! Spotting a chunk that is really a page of numbered notes.
//!
//! Footnotes and endnotes are citations in disguise: they read as body text,
//! rank as body text, and answer a query with a publisher and a page number.
//! Flagging them lets a consumer treat them as the bibliography they are
//! without moving them out of the body, where their page provenance lives.

use std::sync::LazyLock;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;

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
    FancyRegex::new(
        r"(?xm)
    (?: ^[ \t]{0,8} (?:-[ \t]+)?      # line start, optional '- ' list bullet
      | (?<=\s\s)                     # or after a 2+ space run
      | (?<=\ -\ )                    # or after ' - ' (inline list separator)
    )
    (?:
        \d{1,3} \\?[.)] [ \t]+ \S                 # plain '12. ' or escaped '12\. '
      | \[ \d{1,3} \] [ \t]* \S                   # bracket-numbered '[12] Author'
      | \d{1,3} [ \t]+ \S                         # bare '12 ' from a page bottom
      | [¹²³⁰-⁹]+ \s* \S                          # unicode superscript marker
      | (?:fn|ftn|note) \d{1,4} [.)] [ \t]+ \S    # id-style 'fn39. '
      | (?:<span[^>]*>\s*)*                       # outer span wrap
        <a\b[^>]*> \s* (?:<span[^>]*>\s*)* (?:<sup>\s*)?
            \d{1,3} \s* [.)]?
        \s* (?:</sup>\s*)? (?:</span>\s*)* </a>
        (?:\s*</span>)* \s* [.)]?                 # anchor forms
      | \[ \s* (?:<span[^>]*>)? \s* \d{1,3}
        \s* (?:</span>)? \s* \] \( \#[^)]* \)     # [42](#...)
      | <sup> \s* \d{1,3} \s* </sup>              # bare sup note
    )",
    )
    .expect("a valid pattern")
});

/// What makes a note bibliographic rather than discursive.
static CITE_SIGNAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
    \((?:[^()]*\b)?(?:1[5-9]|20)\d{2}\)   # parenthesised year: (Oxford, 1998)
  | \bpp?\.\s*\d                          # p. 12 / pp. 12-34
  | \bvol\.\s*\d                          # vol. 2
  | \bIbid\b | \bop\.\s*cit\b | \bloc\.\s*cit\b
  | \bet\s+al\b
  | \b(?:University|Clarendon|Academic)\s+Press\b | \bPress,\s
  | \beds?\.\s | \btrans\.\s
  | \*[^*\n]{3,80}\*\s*,?\s*\d            # short form: *Title*, 183
  | <span[^>]*italic[^>]*>                # HTML-italic title survives conversion
  | ,\s+\d{1,4}\s*[.;]                    # trailing page: '..., 183.'
  | https?://                             # web citations (URLs, DOIs)",
    )
    .expect("a valid pattern")
});

/// Whether a chunk carries footnote or endnote citation material.
///
/// The structural signal first, when the source had one. Otherwise each note
/// entry runs from its marker to the next one, capped at a few hundred
/// characters, and enough of them have to look like citations.
pub fn looks_like_citations(text: &str) -> bool {
    if text.contains(SENTINEL) {
        return true;
    }

    let starts: Vec<usize> = NOTE_MARKER
        .find_iter(text)
        .filter_map(Result::ok)
        .map(|found| found.start())
        .collect();
    if starts.len() < MIN_CITED_NOTES {
        return false;
    }

    let mut cited = 0;
    for (at, start) in starts.iter().enumerate() {
        let end = starts.get(at + 1).copied().unwrap_or(text.len());
        let entry: String = text[*start..end].chars().take(ENTRY_WINDOW).collect();
        if CITE_SIGNAL.is_match(&entry) {
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
