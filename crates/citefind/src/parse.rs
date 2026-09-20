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

/// The same, but only where it follows something. A title with a colon in
/// it — `Subordination and Equivalence: The Nature and Role of Woman` — is
/// an imprint by the pattern above when the pattern may match at the start.
static IMPRINT_AFTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[(.,;]\s*[\p{Lu}][^:;()\d]{1,40}:\s*[^;()]{2,80}").expect("a valid pattern")
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

/// What a title never is: an opening that is a page or a volume number, or a
/// run that carries a page reference or a semicolon — both of which mean two
/// entries were run together, or that this was never an entry at all.
static NOT_A_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?ix)
      ^[\d\p{Pd}]                      # opens on a page or a volume number
    | \bpp?\.\s*\d                     # carries a page reference
    | ;                                # two entries run together
    | \bibid\b
    | \\                               # an escape conversion left behind
    "#,
    )
    .expect("a valid pattern")
});

/// A reference to chapter and verse: `Matthew 4:17`, `1 Peter 2:6-8`.
///
/// A book of the Bible is not a work in a bibliography — nobody is looking
/// for the publisher of Galatians — and a page of them is what the notes of
/// a theological book largely are.
static VERSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        ^(?:(?:also|cf|see|and|vv?)\.?\s+)*      # 'Cf.', 'also cf.', 'see'
         (?:[1-4]\s+)?                          # 1 Peter, 2 Corinthians
         (?:\p{L}[\p{L}.]{2,14}\.?\s+)?         # the book, where it is named
         \d{1,3}(?:[:.]\d{1,3})?                # chapter, and verse
         (?:[-–]\d{1,3})?                       # through to another
        \.?$",
    )
    .expect("a valid pattern")
});

/// A note number sitting inside a run of verses, which is where conversion
/// leaves it when several notes share one line.
static EMBEDDED_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s\d{1,3}\.\s").expect("a valid pattern"));

/// A contributor's note: a name, and then what they are. It opens exactly
/// as a Chicago note does — `S. Marc Cohen, ...` — and the verb gives it
/// away. These sit in the same back matter as the endnotes and are not
/// references to anything.
static BIOGRAPHY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\*{0,2}\p{Lu}[^*\n]{2,60}?\*{0,2}\s+(?:is|was)\s+(?:a|an|the|professor|emeritus|emerita|lecturer|reader|assistant|associate|senior|currently|director|head|editor|author)\b",
    )
    .expect("a valid pattern")
});

/// What a browser prints at the foot of a page: the address it came from,
/// which page of how many, and when it was printed. A book assembled out of
/// printed web pages carries one on every page, and none of them is a
/// reference to anything.
static PRINT_FURNITURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix) ^ https?://\S+
        (?: \s* \( \s* \d+ \s+ of \s+ \d+ \s* \) )?   # (54 of 100)
        (?: \s* \[ [^\]]{0,40} \] )?                    # [4/7/2002 1:48:08 PM]
        \s* $",
    )
    .expect("a valid pattern")
});

/// A short cite: a surname, a year, and at most a page. `Spade 1980, 12.`
/// and `Trentman 1970; Spade 1980.` name a work the entry does not — the
/// full one is elsewhere in the same bibliography — so there is nothing in
/// them to read and nothing to ask anybody about.
static SHORT_CITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix) ^ (?:(?:see|cf\.?|and)\s+)?
        \p{Lu}[\p{L}'’\-]{1,24}(?:\s+(?:and|&)\s+\p{Lu}[\p{L}'’\-]{1,24})?
        \s+ (?:1[4-9]|20)\d{2} [a-z]?
        (?: \s*(?:,|\s+and)\s* (?:1[4-9]|20)\d{2}[a-z]? )*
        (?: \s* ,\s* (?:pp?\.\s*)? \d{1,4}(?:[-–]\d{1,4})? )?
        \.? $",
    )
    .expect("a valid pattern")
});

/// A name and a locator with nothing between them: `Lowrie and Fletcher,
/// p. 133`, `Anderson, p. 400`. Like a short cite with a year, this points
/// at a work it does not name.
static BARE_CITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix) ^ (?:(?:see|cf\.?|and|also)\s+)*
        \p{Lu}[\p{L}'’\-]{1,24}
        (?:\s+(?:and|&)\s+\p{Lu}[\p{L}'’\-]{1,24})?
        \s*,\s* (?:pp?\.|ff?\.|vol\.|no\.)\s*
        \d{1,5}(?:[-–—]\d{1,5})? \.? $",
    )
    .expect("a valid pattern")
});

/// A note that keys a topic to a short cite: `Mate selection: Buss, 1989;
/// Symons, 1979.` The topic is the author's own words about their own
/// argument; the cites name works given in full in the bibliography.
static KEYED_CITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix) ^ [^:;,]{3,60} :          # the topic, in the note's own words
        (?: \s* (?:and\s+|see\s+)?
            \p{Lu}[\p{L}'\u{2019}\-]{1,24}          # a surname
            (?: \s+ (?:and|&) \s+ \p{Lu}[\p{L}'\u{2019}\-]{1,24}
              | \s+ et \s+ al\.? )?                 # or two, or et al.
            \s*,\s* (?:1[4-9]|20)\d{2} [a-z]?       # and the year
            \s* [;,]? )+ \.? \s* $",
    )
    .expect("a valid pattern")
});

/// A text cited by the critical series it is printed in rather than by its
/// own name: `PG 4: 264`, `CSEL 34.2, 12`, `NPNF 2/5: 430`. Migne and the
/// rest are shelves, not works, and the work is not named here.
static SERIES_CITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x) ^ (?:PG|PL|CSEL|CCSL|CCSG|SC|NPNF|ANF|GCS|PO|CSCO|ACO|ACW|FC|LCL|SVS)
        \s* \d{0,3} (?:[/.]\d{1,3})* \s* :? \s*
        [\d\s,:.–-]* \.? $",
    )
    .expect("a valid pattern")
});

/// What a note that thanks people opens with. These sit among the endnotes,
/// name a dozen people, and cite nothing.
static ACKNOWLEDGEMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:(?:my\s+|with\s+)?thanks\b|i\s+(?:am\s+(?:grateful|indebted)|thank|should\s+like\s+to\s+thank|owe)|this\s+(?:paper|chapter|essay|paper)\s+(?:was|is|grew)|an\s+earlier\s+version|parts?\s+of\s+this)",
    )
    .expect("a valid pattern")
});

/// How a translator or an editor signs a note of their own: an em dash and
/// their role, at the very end. `Latin: "nature of the thing." — Ed.` is a
/// gloss on the text in front of it and cites nothing.
static GLOSS_SIGNATURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[—–-]{1,2}\s*(?:ed|tr|trans|transl|a\.\s*n|au)\.?\s*$")
        .expect("a valid pattern")
});

/// An editor's or translator's credit where an author's name would be:
/// `Barratt, Alexandra, ed. Women's Writing in Middle English.` The role is
/// not a sentence stop, so the name and the title run together into one
/// segment and neither is read.
static EDITOR_FORM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^([^.]{2,60}),\s+(?:eds?|trans|comps?|rev)\.\s+(\S.*)$")
        .expect("a valid pattern")
});

/// A page reference or a bracketed imprint: what a citation has and a remark
/// about the argument does not.
static CITATION_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
    // A page range after a comma counts even with no `pp.` in front of it:
    // `edited by H.J. Störig, 38-70. Darmstadt:` is a chapter in a book, and
    // six sentences long because German titles are.
    Regex::new(r"(?i)\bpp?\.\s*\d|\bvol\.\s*\d|\bno\.\s*\d|,\s*\d{1,5}\s*[-–—]\s*\d{1,5}\b")
        .expect("a valid pattern")
});

/// How many words a reference runs to before it is prose.
///
/// The longest real entry in the corpus sample — a chapter in an edited
/// volume, with two titles, an editor, an imprint and a page range — is
/// under sixty. A hundred and twenty is generous and still nowhere near a
/// paragraph of body text.
const MAX_WORDS: usize = 120;

/// How many sentences a reference runs to before it is prose.
///
/// A full entry in the heaviest style — names, title, an edited volume, an
/// imprint, pages — is five. A discursive endnote runs on past that, and the
/// three it is checked against are the signals a citation carries anyway.
const MAX_SENTENCES: usize = 5;

/// A credit standing where the container would be: `trans. E. B. Ashton`,
/// `edited by Mary Douglas`. A role is not a journal and not a publisher.
static ROLE_CREDIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:trans(?:lated)?|ed(?:ited)?|eds|comp(?:iled)?|rev(?:ised)?|intro(?:duction)?|with)\b\.?(?:\s+by\b)?")
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
    Regex::new(
        r"(?x) ^([^,()]{2,60}),\s+([^()]{4,300}?)\s*
          \( ( [^()]* (?: (?:1[4-9]|20)\d{2}[a-z]? | :\s*\p{Lu} ) [^()]* ) \)",
    )
    .expect("a valid pattern")
});

/// A volume or part trailing a title: `…, Vol. 2`, `…, bk. iv`.
static TRAILING_VOLUME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i),\s*(?:vol|bk|pt|sect?)\.?\s*[\divxlcdm]+\s*$").expect("a valid pattern")
});

/// The word that introduces a locator, which is where the name stops.
static LOCATOR_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:pp?|ff?|vol|bk|ch|no|n|sec)\.?,?$").expect("a valid pattern")
});

/// A locator: where in the work, rather than what the work is. A page, a
/// range, a volume in roman numerals, a folio. These trail a short note and
/// are not part of its title.
static LOCATOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix) ^ (?:pp?\.|ff?\.|vol\.|bk\.|ch\.|no\.|n\.|sec\.)?\s*
        (?: \d{1,5}(?:[-–—]\d{1,5})? | [ivxlcdm]+ (?:[-–—][ivxlcdm]+)? )
        \s* (?:n\.?\s*\d{1,3})? \.? $",
    )
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
    Regex::new(
        r"(?ix)
      \b(?:1[4-9]|20)\d{2}\b | \bpp?\.\s*\d | https?:// | \b10\.\d{4}
    | \bed\. | \btrans\. | \bvol\. | \bno\. | :\s*\d
    | \bpress\b | \buniversity\b | \bjournal\b
    | ,\s*\d{1,5}(?:[-–—]\d{1,5})?\s*\.?$   # a short note ends on its page
    ",
    )
    .expect("a valid pattern")
});

/// A rule of dashes standing in for the author of the entry above, which is
/// how every style avoids printing the same name forty times running.
static REPEATED_AUTHOR: LazyLock<Regex> = LazyLock::new(|| {
    // Pandoc escapes an underscore, so a rule of them arrives as
    // `\_\_\_\_\_` rather than `_____`.
    Regex::new(r"^\s*(?:[—–]+|-{3,}|(?:\\?_){3,})").expect("a valid pattern")
});

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
    if is_scripture(text)
        || BIOGRAPHY.is_match(text)
        || PRINT_FURNITURE.is_match(text)
        || ACKNOWLEDGEMENT.is_match(text)
        || GLOSS_SIGNATURE.is_match(text)
    {
        return None;
    }
    // A short cite points at a full entry elsewhere, the way `ibid.` points
    // at the note above. The author and the year it does give are kept.
    // A keyed cite names no author of its own — `Mate selection:` is the
    // note's own words — so nothing is carried off it but the year.
    if is_keyed_cite(text) {
        out.continuation = true;
        out.year = year(text);
        return Some(out);
    }
    if is_short_cite(text) {
        out.continuation = true;
        out.year = year(text);
        out.pages = pages(text);
        out.authors = short_cite_authors(text);
        return Some(out);
    }
    // A note that runs on past a few sentences, or past a couple of hundred
    // words, is a remark about the argument. The LLM path this replaces was
    // told to skip those; a parser has to decide.
    //
    // A page reference or an identifier buys an exemption, because a note
    // that cites a work and then discusses it is still a citation. An
    // imprint does not: a colon in running prose — `his 1945 report,
    // Science: The Endless Frontier` — is shaped exactly like `Place:
    // Publisher`, and used to let whole paragraphs of body text through.
    let exempt = CITATION_ANCHOR.is_match(text) || !ids(text).is_empty();
    if !exempt
        && (sentences(text).len() > MAX_SENTENCES || text.split_whitespace().count() > MAX_WORDS)
    {
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
        // Quotation marks count as the title only where they open the tail.
        // Further in they are a quotation inside a title, not the title.
        // Whether a span opens the tail: it begins after the year, and what
        // sits between the two is punctuation and space. Asked that way
        // rather than by counting bytes — a curly quotation mark is three of
        // them, so a tolerance of two silently missed every author-date
        // entry that set its title in curly quotes, which is most of them.
        let opens = |at: Option<(usize, usize)>| {
            at.is_some_and(|at| {
                at.0 >= year_at.1
                    && text[year_at.1..at.0]
                        .chars()
                        .all(|c| c.is_whitespace() || ".,:;'\u{2018}\u{201c}\"".contains(c))
            })
        };
        let after_year = |at: (usize, usize)| at.0 >= year_at.1;

        // Whatever is marked up at the head of the tail is the work cited.
        // Italics stand for the journal in `Peirce (1878). Plain title.
        // *Popular Science Monthly*` — where they come after the title — and
        // for the title itself in `Goffman (1963). *Stigma* Penguin`, where
        // they open it. Which it is depends on where they sit, not on what
        // they are.
        let head = match (opens(quoted_at), opens(emphasised_at)) {
            (true, _) => quoted.clone().zip(quoted_at),
            (_, true) => emphasised.clone().zip(emphasised_at),
            _ => None,
        };

        if let Some((title, at)) = head {
            out.title = Some(title);
            out.container = match (&emphasised, emphasised_at) {
                (Some(book), Some(book_at)) if book_at.0 >= at.1 => tidy_container(book),
                _ => container(text, &text[at.1..]),
            };
        } else {
            let parts = sentences(tail);
            // `…, Subordination and Equivalence: …, Washington: University
            // Press of America.` has no stop between the title and the
            // imprint, so the imprint is where the title ends.
            out.title = parts.first().map(|first| {
                let cut = IMPRINT_AFTER
                    .find(first)
                    .map_or(first.len(), |found| found.start());
                tidy_title(&strip_markup(&first[..cut]))
            });
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
    } else if let Some(found) = editor_form(text)
        .or_else(|| note_form(text))
        .or_else(|| comma_form(text))
        .or_else(|| short_note(text))
    {
        out.authors = found.0;
        out.title = found.1;
        out.container = found.2;
    } else {
        let (found_authors, found_title, found_container) = by_sentence(text);
        out.authors = found_authors;
        out.title = found_title;
        out.container = found_container;
    }
    // A title the reading refused comes back empty rather than absent,
    // because the refusal happens where the trimming does. An empty title is
    // an absent one, and `unsettled` should say so.
    if out.title.as_deref().is_some_and(str::is_empty) {
        out.title = None;
    }

    // `See R. Hirzel, Die Person (1914), p. 7.` reads as an author-date entry
    // — the head is all capitalised words and the year is in brackets — and
    // then yields no title, because the title is sitting inside what was
    // taken for the names. A note that names a work is the better reading of
    // the same line, and having a title is how you can tell.
    if out.title.is_none() {
        if let Some((names, title, container)) = note_form(text) {
            if title.is_some() {
                out.authors = names;
                out.title = title;
                out.container = container.or(out.container.take());
            }
        }
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
    // A span the style marked as a title is a title, whatever it opens on:
    // `*1 & 2 Thessalonians*` is a book of the Bible, not a page number.
    let cleaned = tidy_marked_title(&strip_markup(inner.as_str()));
    match cleaned.is_empty() {
        true => (None, None),
        false => (Some(cleaned), Some((whole.start(), whole.end()))),
    }
}

/// An author-date opening with no brackets round the year: `Altmann, M.
/// 1951, A Study of Behaviour`. Common in the sciences and in older German
/// and Russian bibliographies.
static NAME_YEAR: LazyLock<Regex> = LazyLock::new(|| {
    // `Schleiermacher, Friedrich. 1813/1963.` gives the year it was written
    // and the year of the edition used. The first is the one that dates the
    // work; the second still has to be got past.
    Regex::new(r"^(.{2,80}?)[,.]?\s+((?:1[4-9]|20)\d{2})[a-z]?(?:\s*/\s*(?:1[4-9]|20)\d{2}[a-z]?)?\s*[,.:]\s+")
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
    // A rule of dashes stands in for the author of the entry above, and an
    // author-date entry may open on one: `—— (1994), Christianizing Homer`.
    // Only a bare rule, though: `\_\_\_\_\_. "Die Frage nach der Echtheit."
    // *Theologische Blätter* 20 (1941)` opens on one too, and everything
    // between it and the year is the title and the journal.
    let bare_rule = REPEATED_AUTHOR.find(head).is_some_and(|found| {
        head[found.end()..]
            .trim_matches(|c: char| ".,; ".contains(c))
            .is_empty()
    });
    if head.chars().count() > 120 || !(reads_as_names(head) || bare_rule) {
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
    // `(Tübingen, 1933)` names a place and a year; only a bracket with a
    // colon or a publisher's name in it names a publisher.
    let imprint = found[3].trim();
    let publisher = match imprint.contains(':') || PUBLISHER_WORD.is_match(imprint) {
        true => tidy_container(imprint),
        false => None,
    };
    Some((names, Some(title), publisher))
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

    // `Miller, David` is the usual opening, and `Procopius` on its own is the
    // other one. Whichever reads as a name, the title starts after it.
    let pair = format!("{},{}", parts[0], parts[1]);
    let (names, used) = match reads_as_names(&pair) {
        true => (authors(&pair), 2),
        false if reads_as_names(parts[0]) => (authors(parts[0]), 1),
        false => return None,
    };
    if names.is_empty() {
        return None;
    }

    // The imprint is whichever comma-part carries the colon, counted in
    // commas rather than matched by a pattern: a pattern for `Place:
    // Publisher` starts at the comma before the place, which is exactly
    // where the title ends, and the two cannot both have it. The last such
    // part, because a title has a colon in it as often as an imprint does —
    // and if the only one is inside the title, the colon says nothing about
    // where the imprint is and the year does.
    let colon = parts
        .iter()
        .enumerate()
        .rev()
        .find(|(at, part)| *at > used && part.contains(':'))
        .map(|(at, _)| at);
    let at = match colon {
        Some(at) => at,
        None => {
            // `Cheyne, T. K., Founders of Old Testament Criticism, London,
            // 1893.` — the year is last and the place before it, which is
            // not a publisher and is not kept.
            let last = parts.last()?.trim().trim_end_matches('.');
            if !looks_like_a_year(last) {
                return None;
            }
            parts.len() - 2
        }
    };
    if at <= used {
        return None;
    }

    let title = tidy_title(&strip_markup(&parts[used..at].join(",")));
    if title.chars().count() < 4 {
        return None;
    }
    // Where the colon was is a publisher; where the year was is a place.
    let publisher = match parts[at].contains(':') {
        true => tidy_container(parts[at]),
        false => None,
    };
    Some((names, Some(title), publisher))
}

/// Read an entry whose author is credited as its editor.
///
/// `Barratt, Alexandra, ed. Women's Writing in Middle English. London:
/// Longman, 1992.` — `ed.` is an abbreviation rather than a sentence stop,
/// so the name and the title are one segment until the role is taken out.
fn editor_form(text: &str) -> Option<(Vec<String>, Option<String>, Option<String>)> {
    let found = EDITOR_FORM.captures(text.trim())?;
    let names = authors(&found[1]);
    if names.is_empty() {
        return None;
    }
    let rest = found[2].trim();
    let title = tidy_title(&strip_markup(sentences(rest).first()?));
    if title.chars().count() < 4 {
        return None;
    }
    Some((names, Some(title), container(text, rest)))
}

/// Read a short note: a surname, a short title, and where in it.
///
/// `Unger, Dutch Shipbuilding, 33-4` and `Sjoerds, Algemene Beschryvinge, i,
/// f I, 501-2.` — the form a history book uses for every citation after the
/// first, once the full entry has been given. There is no year in it and no
/// publisher; what is there is a name, a title, and a page.
///
/// The trailing locators are not part of the title. Neither is a year: an
/// entry ending on one is an imprint, and belongs to `comma_form`.
fn short_note(text: &str) -> Option<(Vec<String>, Option<String>, Option<String>)> {
    let text = LEAD_IN.replace(text.trim(), "");
    if sentences(&text).len() > 1 {
        return None;
    }
    let parts: Vec<&str> = text
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    // It has to end on a locator, or it is some other kind of run entirely.
    let last = parts.last()?.trim_end_matches('.');
    if !LOCATOR.is_match(last) || looks_like_a_year(last) {
        return None;
    }
    // `Krochmal, N., Moreh Nebukhe ha-Zeman` puts the initials in a part of
    // their own; they belong to the name, not to the title.
    let named = match parts.get(1).is_some_and(|part| is_initials(part)) {
        true => 2,
        false => 1,
    };
    let names = authors(&parts[..named].join(", "));
    if names.is_empty() {
        return None;
    }
    // The title runs from after the name to the first locator.
    let end = parts
        .iter()
        .skip(named)
        .position(|part| LOCATOR.is_match(part.trim_end_matches('.')))
        .map(|at| at + named)
        .unwrap_or(parts.len());
    if end <= named {
        return None;
    }
    let title = tidy_title(&strip_markup(&parts[named..end].join(", ")));
    match title.chars().count() >= 4 {
        true => Some((names, Some(title), None)),
        false => None,
    }
}

/// Whether a run is nothing but initials: `N.`, `T. K.`, `J. R. R.`
fn is_initials(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    !words.is_empty()
        && words.len() <= 4
        && words.iter().all(|word| {
            let core = word.trim_end_matches('.');
            core.chars().count() == 1 && core.chars().all(char::is_uppercase)
        })
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

/// Whether a note is one or more topics keyed to short cites and nothing
/// else: `Mate selection: Buss, 1989; Symons, 1979. Jealousy: Buss, 2000.`
fn is_keyed_cite(text: &str) -> bool {
    let parts = sentences(text);
    !parts.is_empty()
        && parts
            .iter()
            .all(|part| KEYED_CITE.is_match(part.trim_end_matches('.')))
}

/// Whether every part of an entry is a short cite and nothing more.
fn is_short_cite(text: &str) -> bool {
    let parts: Vec<&str> = text
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    !parts.is_empty()
        && parts
            .iter()
            .all(|part| SHORT_CITE.is_match(part) || BARE_CITE.is_match(part))
}

/// The surnames a short cite names, in the order it names them.
fn short_cite_authors(text: &str) -> Vec<String> {
    text.split(';')
        .filter_map(|part| {
            let part = LEAD_IN.replace(part.trim(), "").trim().to_string();
            // Up to the year or the locator, whichever the cite carries.
            let name: Vec<&str> = part
                .split_whitespace()
                .take_while(|word| {
                    !word.chars().next().is_some_and(|c| c.is_ascii_digit())
                        && !LOCATOR_WORD.is_match(word)
                })
                .collect();
            let name = name.join(" ");
            let name = name.trim_end_matches([',', '.', ';', ' ']);
            match name.is_empty() {
                true => None,
                false => Some(restore_initial(name)),
            }
        })
        .collect()
}

/// Whether an entry is chapter and verse and nothing else.
///
/// Several of them separated by semicolons is still a scripture reference
/// and still not a work: `Cf. Deuteronomy 18:15-19; Isaiah 53; Acts 1:6`.
fn is_scripture(text: &str) -> bool {
    let text = text.trim_start_matches(['.', ' ']);
    let text = EMBEDDED_MARKER.replace_all(text, ";");
    let parts: Vec<&str> = text
        .split([';', ','])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty()
        || !parts
            .iter()
            .all(|part| VERSE.is_match(part) || SERIES_CITE.is_match(part))
    {
        return false;
    }
    // A run of bare numbers is not scripture just because it could follow
    // from some book named elsewhere. One of them has to name the book.
    parts
        .iter()
        .any(|part| part.chars().any(char::is_alphabetic))
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

/// The same, for a run the style itself marked as a title.
///
/// The rule against a title opening on a number is about a page reference
/// picked up by mistake. Inside quotation marks or italics there is no
/// mistake to make: `*1 & 2 Thessalonians*` is what the entry says the work
/// is called.
fn tidy_marked_title(raw: &str) -> String {
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = text.trim_end_matches('\\');
    let text =
        text.trim_matches(|c: char| ",.;:'\u{2018}\u{2019}".contains(c) || c.is_whitespace());
    let without_volume = TRAILING_VOLUME.replace(text, "").into_owned();
    let text = TRAILING_CREDIT.replace(&without_volume, "").into_owned();
    let text = text.as_str();
    let text = text.trim_end_matches(|c: char| ",.;: ".contains(c));
    match MARKED_NOT_A_TITLE.is_match(text) || !quotes_balance(text) {
        true => String::new(),
        false => text.to_string(),
    }
}

/// What a marked-up title still may not be: two entries run together, or a
/// reference to where a work is rather than what it is.
static MARKED_NOT_A_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?ix) \bpp?\.\s*\d | ; | \bibid\b | \\").expect("a valid pattern")
});

/// An editor's or translator's credit following a title: `Saturae XIV, ed.
/// J. D. Duff`. It belongs to the entry, not to the work's name.
static TRAILING_CREDIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i),\s+(?:eds?|trans|translated\s+by|comp)\.?\s+\p{Lu}.*$")
        .expect("a valid pattern")
});

/// Strip the punctuation a style leaves inside its own title delimiters.
fn tidy_title(raw: &str) -> String {
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    // A trailing backslash is a line break conversion left behind, not part
    // of anything.
    let text = text.trim_end_matches('\\');
    // A conversion that lost an opening quotation mark leaves its partner at
    // the end. That is a stray, not evidence the cut went wrong — which is
    // what one in the middle of a run means.
    let text = match text.matches('"').count() == 1 && text.ends_with('"') {
        true => text.trim_end_matches('"'),
        false => text,
    };
    // The quotation marks a style puts round a title are the style's, not the
    // title's, and only the outermost pair is trimmed.
    let text =
        text.trim_matches(|c: char| ",.;:'\u{2018}\u{2019}".contains(c) || c.is_whitespace());
    // `Geschichte des Papsttums, Vol. 2` names the work and then which volume
    // of it. The volume is where in the work, not what the work is called.
    let without_volume = TRAILING_VOLUME.replace(text, "").into_owned();
    let text = TRAILING_CREDIT.replace(&without_volume, "").into_owned();
    let text = text.as_str();
    let text = text.trim_end_matches(|c: char| ",.;: ".contains(c));
    // A run that opens on a page number, or carries one, is two entries run
    // together or a reference to a work rather than the name of one. Better
    // to have no title and say so than to report that as one.
    if NOT_A_TITLE.is_match(text) || !quotes_balance(text) {
        return String::new();
    }
    text.to_string()
}

/// Whether every quotation mark in a run has its partner.
///
/// `What's the Meaning of "This"?` is a title with a quotation inside it.
/// `Mediterraneans." In Rethinking the Mediterranean` is the tail of a title
/// whose opening is on the page before. One quotation mark tells them apart.
fn quotes_balance(text: &str) -> bool {
    let straight = text.matches('"').count();
    let opened = text.matches('\u{201c}').count();
    let closed = text.matches('\u{201d}').count();
    straight % 2 == 0 && opened == closed
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
    // A bracket or a colon belongs to an imprint or a title, never to a
    // name: `Philosophy (Bloomington: Indiana University Press)` is not one.
    if text.contains(['(', ')', ':', '[', ']']) {
        return false;
    }
    // An editor's or translator's credit is a role, not a name.
    if text.to_lowercase().contains("ed.") || text.to_lowercase().contains("trans.") {
        return false;
    }
    // A lowercase word in the middle of a name is a particle, an initial the
    // scan read in lower case, or a sentence — and there are only so many
    // particles.
    let sentence = words.iter().any(|word| {
        let core = word.trim_matches(|c: char| !c.is_alphanumeric());
        core.chars().next().is_some_and(char::is_lowercase)
            && core.chars().count() > 1
            && !PARTICLES.contains(&core.to_lowercase().as_str())
    });
    if sentence {
        return false;
    }
    // `de la Torre, Carlos` opens on a particle and is still a name; what it
    // must not be is a run with no capital in it anywhere.
    let first = words[0].trim_matches(|c: char| !c.is_alphanumeric());
    let opens_well = first.chars().next().is_some_and(char::is_uppercase)
        || PARTICLES.contains(&first.to_lowercase().as_str())
        || first.chars().count() == 1;
    opens_well
        && words
            .iter()
            .any(|word| word.chars().next().is_some_and(char::is_uppercase))
}

/// What the work appeared in: a journal, or a book's publisher.
fn container(whole: &str, after: &str) -> Option<String> {
    // A book names its publisher in an imprint after the title, whether
    // bracketed or not. Only after the title: a title with a colon in it —
    // and half of them have one — would otherwise read as an imprint.
    if let Some(found) = IMPRINT.captures(after) {
        if let Some(publisher) = tidy_container(found[1].trim()) {
            return Some(publisher);
        }
    }
    // A journal sits between the title and the volume, but a credit can sit
    // in front of it — `*Negative Dialectics*, trans. E. B. Ashton` — and a
    // translator is neither a journal nor a publisher.
    let tail = after.trim_start_matches(|c: char| ",.;:\"”'’ ".contains(c));
    let tail = match ROLE_CREDIT.find(tail) {
        Some(found) => match tail[found.end()..].find([',', ';', '(']) {
            Some(at) => tail[found.end() + at + 1..].trim_start(),
            None => "",
        },
        None => tail,
    };
    let end = VOLUME
        .find(tail)
        .map(|found| found.start())
        .or_else(|| tail.find(['(', '.', ';']))
        .unwrap_or(tail.len());
    let found = tidy_container(&tail[..end])?;
    // `(London, 1973)` names a place and a year and no publisher. Reporting
    // the place as the publisher would be a plausible wrong answer, and the
    // place is still there in `text` for anyone who wants it.
    match is_place_of_publication(whole, &found) {
        true => None,
        false => Some(found),
    }
}

/// Whether a candidate is the place a book was published rather than its
/// publisher: it sits inside brackets with the year and nothing else.
fn is_place_of_publication(whole: &str, found: &str) -> bool {
    let needle = format!("({found}, ");
    let Some(at) = whole.find(&needle) else {
        return false;
    };
    let rest = &whole[at + needle.len()..];
    let year: String = rest.chars().take_while(char::is_ascii_digit).collect();
    year.len() == 4 && rest[year.len()..].starts_with(')')
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
        if matches!(bytes[at], b'.' | b'?' | b'!') {
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
            // A question mark or an exclamation is always a stop; a full
            // stop is one unless it belongs to an initial or an abbreviation.
            let stop = bytes[at] != b'.' || (!initial && !abbreviated);
            if stop && followed_by_space {
                out.push(text[start..=at].trim_end_matches('.').trim());
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
    fn an_author_date_title_may_be_set_in_curly_quotes() {
        // A curly quotation mark is three bytes, and a tolerance counted in
        // bytes missed every entry set this way — which is most of them.
        let chapter = read(
            "Shaw, Joseph W., and Marjatta Luton. 2000. \u{201c}The Foreshore at \
             Akrotiri.\u{201d} In *Wall Paintings of Thera*, 453-66.",
        );
        assert_eq!(chapter.authors, ["Shaw, Joseph W.", "Marjatta Luton"]);
        assert_eq!(chapter.title.as_deref(), Some("The Foreshore at Akrotiri"));
        assert_eq!(
            chapter.container.as_deref(),
            Some("Wall Paintings of Thera")
        );

        let article = read(
            "James, Liz. 2003. \u{201c}Color and Meaning in Byzantium,\u{201d} \
             *Journal of Early Christian Studies* 11.2: 223-233.",
        );
        assert_eq!(
            article.title.as_deref(),
            Some("Color and Meaning in Byzantium")
        );
        assert_eq!(
            article.container.as_deref(),
            Some("Journal of Early Christian Studies")
        );
    }

    #[test]
    fn an_author_date_entry_may_open_on_a_rule_of_dashes() {
        let found = read(
            "\u{2014}\u{2014} (1994), Christianizing Homer: The Odyssey, Plato, \
             and the Acts of Andrew (New York: Oxford University Press).",
        );
        assert_eq!(found.year, Some(1994));
        assert_eq!(
            found.title.as_deref(),
            Some("Christianizing Homer: The Odyssey, Plato, and the Acts of Andrew")
        );
    }

    #[test]
    fn a_rule_of_dashes_stands_for_the_author_and_nothing_else() {
        // The dash opens the entry, and everything between it and the year
        // is the title and the journal — not more of the author.
        let found = read(
            "\\_\\_\\_\\_\\_. \u{201c}Die Frage nach der Echtheit von Matth. 16, \
             17-19.\u{201d} *Theologische Blatter* 20 (1941): 26-30.",
        );
        assert_eq!(
            found.title.as_deref(),
            Some("Die Frage nach der Echtheit von Matth. 16, 17-19")
        );
        assert_eq!(found.container.as_deref(), Some("Theologische Blatter"));
        assert_eq!(found.year, Some(1941));
    }

    #[test]
    fn a_reprint_gives_two_years_and_an_editor_takes_the_author_slot() {
        // `1813/1963` is the year it was written and the year of the
        // edition used; the page range says it is a chapter in a book, which
        // is why six sentences of German do not make it prose.
        let reprint = read(
            "Schleiermacher, Friedrich. 1813/1963. \"Ueber die verschiedenen \
             Methoden des Uebersezens\". In Das Problem des Uebersetzens, \
             edited by H.J. Stoerig, 38-70. Darmstadt: Wissenschaftliche \
             Buchgesellschaft.",
        );
        assert_eq!(reprint.authors, ["Schleiermacher, Friedrich"]);
        assert_eq!(
            reprint.title.as_deref(),
            Some("Ueber die verschiedenen Methoden des Uebersezens")
        );
        assert_eq!(reprint.year, Some(1813));

        // `ed.` is an abbreviation rather than a stop, so the name and the
        // title stayed one segment and neither was read.
        let editor = read(
            "Barratt, Alexandra, ed. Women's Writing in Middle English. \
             London: Longman, 1992.",
        );
        assert_eq!(editor.authors, ["Barratt, Alexandra"]);
        assert_eq!(
            editor.title.as_deref(),
            Some("Women's Writing in Middle English")
        );
        assert_eq!(editor.container.as_deref(), Some("Longman"));
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
        // Opens exactly as a Chicago note does — a name, a comma — and then
        // says what the person is. These sit in the same back matter as the
        // endnotes and refer to nothing.
        for bio in [
            "Thomas L. Sheridan, SJ, is Professor Emeritus of Theology at St \
             Peter's College. He is the author of Newman on Justification (1967).",
            "**S. Marc Cohen** is Professor of Philosophy at the University of \
             Washington, where he teaches the history of ancient philosophy.",
            "William J. Gavin was a Professor of Philosophy at the University \
             of Southern Maine for 34 years.",
        ] {
            assert!(one(bio, None).is_none(), "{bio}");
        }
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
    fn a_translators_note_signs_itself_and_cites_nothing() {
        for gloss in [
            "Latin: \"nature of the thing.\" — Ed.",
            "Reference to the wars between Athens and Sparta, 431-404 BC. — Ed.",
            "Meaning here names, dates and numbers, in the usual sense. — Tr.",
        ] {
            assert!(one(gloss, None).is_none(), "{gloss}");
        }
        // A note that cites a work is still a citation, sign-off or not.
        let real = read("See R. Hirzel, Die Person (1914), p. 7.");
        assert_eq!(real.authors, ["R. Hirzel"]);
        assert_eq!(real.title.as_deref(), Some("Die Person"));
        assert_eq!(real.year, Some(1914));
    }

    #[test]
    fn a_short_note_names_a_work_without_a_year_or_a_publisher() {
        // The form a history book uses for every citation after the first:
        // a surname, a short title, and where in it. No year, no imprint.
        let found = read("Unger, Dutch Shipbuilding, 33-4.");
        assert_eq!(found.authors, ["Unger"]);
        assert_eq!(found.title.as_deref(), Some("Dutch Shipbuilding"));
        assert_eq!(found.pages.as_deref(), Some("33-4"));
        assert_eq!(found.year, None);

        // The locators trailing it are not part of the title.
        let volume = read("Sjoerds, Algemene Beschryvinge, i, f I, 501-2.");
        assert_eq!(volume.title.as_deref(), Some("Algemene Beschryvinge"));

        // The quotation marks round a short article title are the style's.
        let quoted = read("Schoeck, 'Agricola and Erasmus', 181, 184.");
        assert_eq!(quoted.title.as_deref(), Some("Agricola and Erasmus"));
    }

    #[test]
    fn an_entry_ending_on_a_year_is_an_imprint_and_not_a_page() {
        // `short_note` must not take `Oxford, Clarendon Press, 1998` for a
        // name, a title and a page.
        let found = read("Miller, David, On Nationality, Oxford: Clarendon Press, 1995");
        assert_eq!(found.authors, ["Miller, David"]);
        assert_eq!(found.title.as_deref(), Some("On Nationality"));
        assert_eq!(found.container.as_deref(), Some("Clarendon Press"));
        assert_eq!(found.year, Some(1995));
    }

    #[test]
    fn the_chicago_forms_a_history_or_a_theology_book_sets_its_notes_in() {
        // A place and a year, no colon and no publisher word anywhere.
        let place = read("Cheyne, T. K., Founders of Old Testament Criticism, London, 1893.");
        assert_eq!(place.authors, ["Cheyne, T. K."]);
        assert_eq!(
            place.title.as_deref(),
            Some("Founders of Old Testament Criticism")
        );
        assert_eq!(place.year, Some(1893));
        // London is where it was published, not who published it.
        assert_eq!(place.container, None);

        // A volume is where in the work, not what the work is called.
        let volume =
            read("Erich Caspar, Geschichte des Papsttums, Vol. 2 (Tubingen, 1933), pp. 587-89");
        assert_eq!(volume.title.as_deref(), Some("Geschichte des Papsttums"));
        assert_eq!(volume.container, None);
        assert_eq!(volume.pages.as_deref(), Some("587-89"));

        // A bracketed imprint need not carry a year.
        let undated =
            read("St Valerian, Homilies (Washington D.C.: Catholic University of America Press).");
        assert_eq!(undated.title.as_deref(), Some("Homilies"));
        assert_eq!(
            undated.container.as_deref(),
            Some("Catholic University of America Press")
        );

        // A title with a colon in it is not an imprint.
        let colon = read(
            "BORRESEN, K. E. (1981), Subordination and Equivalence: The Nature \
             and Role of Woman, Washington: University Press of America.",
        );
        assert_eq!(
            colon.title.as_deref(),
            Some("Subordination and Equivalence: The Nature and Role of Woman")
        );
    }

    #[test]
    fn a_work_cited_by_the_shelf_it_sits_on_is_not_named() {
        // Migne and the rest are series, not works.
        for series in [
            "PG 4: 264, 337, 360, 393, 405; PG 91: 1312-13",
            "NPNF 2/5: 430",
        ] {
            assert!(one(series, None).is_none(), "{series}");
        }
        // A name and a page with nothing between them names no work either.
        let bare = read("Lowrie and Fletcher, p. 133; Anderson, p. 400.");
        assert!(bare.continuation);
        assert_eq!(bare.authors, ["Lowrie and Fletcher", "Anderson"]);
        assert!(bare.missing().is_empty());
    }

    #[test]
    fn a_colon_inside_the_title_does_not_hide_the_imprint() {
        // The only colon here is the title's own, so it says nothing about
        // where the imprint is — and the year at the end does.
        let found = read(
            "Kymlicka, Will, Politics in the Vernacular: Nationalism, \
             Multiculturalism and Citizenship, Oxford, 2001",
        );
        assert_eq!(found.authors, ["Kymlicka, Will"]);
        assert_eq!(
            found.title.as_deref(),
            Some("Politics in the Vernacular: Nationalism, Multiculturalism and Citizenship")
        );
        assert_eq!(found.year, Some(2001));
    }

    #[test]
    fn an_editors_credit_is_trimmed_off_a_title_not_grounds_to_drop_it() {
        let found = read("Juvenal, Saturae XIV, ed. J. D. Duff (Cambridge, 1962), p. 9.");
        assert_eq!(found.authors, ["Juvenal"]);
        assert_eq!(found.title.as_deref(), Some("Saturae XIV"));
        assert_eq!(found.pages.as_deref(), Some("9"));
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
    fn chapter_and_verse_is_not_a_work_in_a_bibliography() {
        for verse in [
            "Matthew 4:17",
            ". Cf. Deuteronomy 18:15-19; Isaiah 53; Acts 1:6; 2:16-36; 1 Peter 2:6-8",
            "1 Peter 2:6-8",
            "Matthew 13:43; cf Exodus 34:29-35",
            "Cf. Matthew 3:7-9; Acts 1:8; 11:1-18; 15:16-17; Galatians 3:14;28",
            "Colossians 1:18,24; Romans 12:5; 1 Corinthians 12:12-13. 19. 2 Corinthians 11:2",
            "Galatians 6:15-16; also cf. John. 1:11-13; Romans 2:28-29; James 1:1 13. Romans 9-11",
        ] {
            assert!(one(verse, None).is_none(), "{verse}");
        }
        // A journal and its volume look similar and are not the same thing.
        assert!(
            one(
                "Altmann, M. 1951, A Study of Behaviour. Sociometry 14, 351-354.",
                None
            )
            .is_some()
        );
    }

    #[test]
    fn what_a_browser_printed_at_the_foot_of_a_page_is_not_a_reference() {
        for junk in [
            "http://www.u.arizona.edu/~chalmers/biblio/1.html (54 of 100) [4/7/2002 1:48:08 PM]",
            "http://www.u.arizona.edu/~chalmers/papers/content95.html",
            "https://example.org/a/b (3 of 9)",
        ] {
            assert!(one(junk, None).is_none(), "{junk}");
        }
        // A URL that is part of a reference still is one.
        let real = read(
            "Chalmers, D. (1996). *The Conscious Mind*. Oxford University Press. http://consc.net/book",
        );
        assert_eq!(real.title.as_deref(), Some("The Conscious Mind"));
    }

    #[test]
    fn a_stray_quotation_mark_at_the_end_is_not_a_cut_title() {
        // Conversion lost the opening mark and left its partner behind.
        let found = read(
            "McClelland, J.L, D.E. Rumelhart, & G.E. Hinton 1986. The appeal \
             of parallel distributed processing\". In Parallel Distributed \
             Processing, MIT Press.",
        );
        assert_eq!(
            found.title.as_deref(),
            Some("The appeal of parallel distributed processing")
        );
        // One in the middle still means the opening is on the page before.
        let cut = read(
            "Mediterraneans.\" In Rethinking the Mediterranean, ed. William V. \
             Harris, 64-93. Oxford: Oxford University Press, 2005.",
        );
        assert_eq!(cut.title, None, "{cut:#?}");
    }

    #[test]
    fn a_note_that_thanks_people_cites_nothing() {
        for note in [
            "Thanks to Ned Block, Frank Jackson, David Kaplan and David Lewis \
             for discussion of these matters over many years.",
            "I am grateful to the seminar at Leiden for pressing me on this, \
             and to two anonymous readers for the Press.",
            "An earlier version of this chapter was given at the 1998 meeting \
             of the Society, and I thank those present.",
        ] {
            assert!(one(note, None).is_none(), "{note}");
        }
    }

    #[test]
    fn a_topic_keyed_to_short_cites_names_no_work_and_no_author() {
        // A notes section that reads `Mate selection: Buss, 1989` is keying
        // the author's own words to works given in full elsewhere. The topic
        // is not a name and there is nothing here to resolve.
        let found = read("Mate selection: Buss, 1989; Symons, 1979. Jealousy: Buss, 2000.");
        assert!(found.continuation);
        assert!(found.authors.is_empty(), "{found:#?}");
        assert!(found.title.is_none());
        assert!(found.missing().is_empty());

        // An entry held together by commas with an imprint at the end looks
        // similar and is a reference.
        let real = read("Miller, David, On Nationality, Oxford: Clarendon Press, 1995");
        assert_eq!(real.authors, ["Miller, David"]);
        assert_eq!(real.title.as_deref(), Some("On Nationality"));
    }

    #[test]
    fn a_title_the_style_marked_may_open_on_a_number() {
        // `*1 & 2 Thessalonians*` is two books of the Bible, not a page.
        let found = read("Bruce, F. F. (1982), *1 & 2 Thessalonians*, Waco, TX: Word.");
        assert_eq!(found.title.as_deref(), Some("1 & 2 Thessalonians"));
    }

    #[test]
    fn a_short_cite_names_a_work_it_does_not_name() {
        // `Spade 1980` points at the full entry elsewhere in the same
        // bibliography, the way `ibid.` points at the note above.
        let found = read("Spade 1980, 12.");
        assert!(found.continuation);
        assert_eq!(found.authors, ["Spade"]);
        assert_eq!(found.year, Some(1980));
        assert_eq!(found.title, None);
        // Nothing to ask anybody about, so nothing does.
        assert!(found.missing().is_empty());

        let several = read("Trentman 1970; Spade 1980.");
        assert!(several.continuation);
        assert_eq!(several.authors, ["Trentman", "Spade"]);

        // Pandoc escapes an underscore, so a rule of them arrives escaped.
        let escaped = read(
            "\\_\\_\\_\\_\\_. *The Community of the Beloved Disciple*. New York: Paulist, 1979.",
        );
        assert_eq!(
            escaped.title.as_deref(),
            Some("The Community of the Beloved Disciple")
        );
        assert!(repeats_author(&escaped.text));

        // One name, two years: `Spade 1975 and 1980.`
        let years = read("Spade 1975 and 1980.");
        assert!(years.continuation);
        assert_eq!(years.authors, ["Spade"]);
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
    fn a_translator_is_not_the_journal_it_stands_in_front_of() {
        let found = read(
            "Theodor W. Adorno, *Negative Dialectics*, trans. E. B. Ashton (London, 1973), 365.",
        );
        assert_eq!(found.title.as_deref(), Some("Negative Dialectics"));
        // `(London, 1973)` names a place and a year and no publisher at all.
        assert_eq!(found.container, None);
        assert_eq!(found.pages.as_deref(), Some("365"));
    }

    #[test]
    fn a_colon_in_running_prose_is_not_an_imprint() {
        // Six sentences of body text, with `his 1945 report, Science: The
        // Endless Frontier` in the middle of it — shaped exactly like
        // `Place: Publisher`, which used to buy the whole paragraph an
        // exemption from the prose test.
        let prose = "Thirdly, not all fields of science were seen as equally \
            relevant to the war effort. Bush himself was an electrical \
            engineer; his chief lieutenants were drawn from the physical \
            sciences. Weapons such as radar drew heavily upon such \
            disciplines. OSRD was busily creating an electronic environment \
            for war. At any rate, physics emerged from the war as the most \
            prestigious discipline. And finally, Bush proposed - most \
            famously in his 1945 report, Science: The Endless Frontier - \
            that Washington continue to play a leading role.";
        assert!(one(prose, None).is_none(), "{prose}");

        // A long entry that cites something is still a citation: the page
        // range is what buys the exemption, not the colon.
        let chapter = read(
            "Heder, Stephen P. 'The Kampuchean-Vietnamese Conflict.' In David \
             W.P. Elliott, ed. *The Third Indochina Conflict.* Boulder: \
             Westview Press. 1981. pp. 21-67.",
        );
        assert_eq!(
            chapter.title.as_deref(),
            Some("The Third Indochina Conflict")
        );
    }

    #[test]
    fn a_remark_about_the_argument_is_not_a_reference() {
        let found = one(
            "Attention is a limited resource. Many psychologists have remarked \
             on this. It follows that what a person attends to is what they \
             come to know. The point is developed at length below. I am \
             grateful to the seminar for pressing me on it.",
            None,
        );
        assert!(found.is_none(), "{found:#?}");
    }

    #[test]
    fn a_page_reference_is_not_a_title() {
        // Two notes run together, or a note that names no work at all.
        let found = read(
            "See Hefele-Leclerq (1909), 72-7, though Diekamp (1899) should have concluded the debate.",
        );
        assert_eq!(found.title, None, "{found:#?}");
        assert!(found.missing().contains(&"title"));
    }

    #[test]
    fn a_title_the_cut_landed_inside_of_is_not_reported() {
        // The chunk began part-way through this entry, so the opening of the
        // title — and its opening quotation mark — are on the page before.
        let found = read(
            "Mediterraneans.\" In Rethinking the Mediterranean, ed. William V. \
             Harris, 64-93. Oxford: Oxford University Press, 2005.",
        );
        assert_eq!(found.title, None, "{found:#?}");
        assert_eq!(found.container.as_deref(), Some("Oxford University Press"));
    }

    #[test]
    fn italics_at_the_head_of_the_tail_are_the_title_and_not_the_journal() {
        // `Peirce (1878). Plain title. *Popular Science Monthly*` puts the
        // journal in italics after the title; `Goodwin (1990). *Title*
        // Publisher` puts the title in them. Where they sit decides which.
        let found = read(
            "Goodwin, F. K. and Jamison, K. R. (1990). *Manicdepressive illness* \
             Oxford University Press, New York.\\",
        );
        assert_eq!(found.authors, ["Goodwin, F. K.", "Jamison, K. R."]);
        assert_eq!(found.year, Some(1990));
        assert_eq!(found.title.as_deref(), Some("Manicdepressive illness"));
    }

    #[test]
    fn a_title_may_quote_something_but_not_half_quote_it() {
        let quoting =
            read("Austin, D. F. 1990. What's the Meaning of \"This\"? Cornell University Press.");
        assert_eq!(
            quoting.title.as_deref(),
            Some("What's the Meaning of \"This\"?")
        );
        // One quotation mark means the opening is on the page before.
        let cut = read(
            "Mediterraneans.\" In Rethinking the Mediterranean, ed. William V. \
             Harris, 64-93. Oxford: Oxford University Press, 2005.",
        );
        assert_eq!(cut.title, None, "{cut:#?}");
    }

    #[test]
    fn an_edition_in_a_title_is_not_an_editors_credit() {
        // `, 2nd ed. (` is part of the title; `, ed. William Harris` is not.
        let found = read(
            "Carter Lindberg, The European Reformations, 2nd ed. (West Sussex, UK: Blackwell, 2010), 161.",
        );
        assert_eq!(
            found.title.as_deref(),
            Some("The European Reformations, 2nd ed")
        );
        assert_eq!(found.container.as_deref(), Some("Blackwell"));
    }

    #[test]
    fn sentences_do_not_stop_at_an_initial_or_an_abbreviation() {
        assert_eq!(
            sentences("Tolkien, J. R. R. The Hobbit, rev. ed. London, 1951."),
            ["Tolkien, J. R. R. The Hobbit, rev. ed. London, 1951."]
        );
    }
}
