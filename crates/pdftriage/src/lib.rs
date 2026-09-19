//! Says whether a PDF's text layer is worth reading, or whether it has to be
//! OCRed.
//!
//! OCR is the expensive step in any ingest: minutes of GPU time a page, and a
//! result that is worse than a text layer the file already had. So the
//! question "is this layer any good" is worth answering carefully, and the
//! thresholds below are measurements rather than guesses — each one is the
//! middle of a gap observed across three thousand real PDFs, with the reason
//! recorded beside it.
//!
//! Everything here is a function over already-extracted text. Running
//! `pdftotext` is the binary's job, so that the judgement can be tested
//! against the numbers without shipping anybody's books.

pub mod decode;
pub mod reflect;
pub mod score;

use corpus_core::TextLayer;

/// Nothing legitimate carries more than two per cent of its characters in the
/// Latin-1 band, and the lowest genuine letter share measured was 0.39 —
/// chess notation, recipe lists, a page index. Both gates sit clear of that
/// by a wide margin, because the cost of a false positive is a needless
/// re-OCR of a book that already reads.
const DEAD_LETTER_SHARE: f64 = 0.25;
const DEAD_LATIN1_SHARE: f64 = 0.35;

/// Non-space characters a page below which a text layer is not really there.
///
/// Chosen from the measured gap, not from a round number. The broken books
/// sit at 3, 86, 94 and 132 characters a page — a scan whose layer holds only
/// the page numbers and the running heads. The lowest-density legitimate
/// books are small-format e-reader PDFs at 258 to 382. Two hundred sits in
/// the middle of that gap with margin on both sides.
const SPARSE_CHARS_PER_PAGE: f64 = 200.0;

/// Function words that make up a large, stable share of any English prose.
///
/// A damaged synthetic-font layer still emits letters, so counting letters
/// proves nothing — but it cannot reproduce these. Healthy layers score 0.25
/// to 0.40; garbled ones score near zero. A tenth sits in the gap with room
/// on both sides.
const COMMON_WORDS: &[&str] = &[
    "a", "all", "an", "and", "are", "as", "at", "be", "been", "but", "by", "for", "from", "had",
    "have", "his", "in", "is", "it", "more", "not", "of", "on", "one", "or", "that", "the",
    "there", "they", "this", "to", "was", "we", "were", "which", "with", "you",
];
const HEALTHY_WORD_RATIO: f64 = 0.10;

/// Characters stripped from the ends of a word before it is looked up.
const WORD_TRIM: &[char] = &['.', ',', ';', ':', '!', '?', '(', ')', '[', ']', '"', '\''];

/// How much text has to be there before a judgement means anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Non-space characters in the opening sample, below which the layer is
    /// treated as absent.
    pub min_chars_present: usize,
    /// Non-space characters in the middle sample, below which damage cannot
    /// be judged.
    pub min_chars_to_judge: usize,
    /// Words needed before the English prose check means anything.
    pub min_words: usize,
    /// Pages below which density says nothing — a handful of pages cannot be
    /// judged that way.
    pub min_pages_for_density: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            min_chars_present: 200,
            min_chars_to_judge: 400,
            min_words: 80,
            min_pages_for_density: 8,
        }
    }
}

/// What was read off a file, and what it means.
#[derive(Debug, Clone, PartialEq)]
pub struct Triage {
    /// The verdict.
    pub layer: TextLayer,
    /// Whether the file has to be OCRed to become searchable.
    pub needs_ocr: bool,
    /// What the numbers were, in the order they were taken.
    pub notes: Vec<String>,
}

/// The text a judgement is made from.
#[derive(Debug, Clone, Copy)]
pub struct Sample<'a> {
    /// Text extracted from the opening pages, which answers whether there is
    /// a layer at all.
    pub opening: &'a str,
    /// Text extracted from the middle of the document, which answers whether
    /// the layer is any good. The middle, because front matter is often a
    /// bare scan in a book whose body carries perfectly good text — and is
    /// often cleanly typeset boilerplate in a book whose body is damaged.
    pub middle: &'a str,
    /// How many pages the middle sample covers.
    pub middle_pages: u32,
    /// Pages in the whole document, when they could be counted.
    pub pages: Option<u32>,
}

/// Judge a text layer.
///
/// The order is not arbitrary. A layer that is not there at all cannot be
/// judged for damage; a layer that is only permuted is recoverable in full
/// and must not be sent for OCR; damage is a stronger statement than
/// thinness, so it is tested first.
pub fn triage(sample: &Sample<'_>, limits: &Limits) -> Triage {
    let mut notes = Vec::new();

    let opening_chars = score::body(sample.opening).len();
    notes.push(format!("{opening_chars} characters on the opening pages"));
    if opening_chars < limits.min_chars_present {
        return Triage {
            layer: TextLayer::Absent,
            needs_ocr: true,
            notes,
        };
    }

    let middle = score::body(sample.middle);
    let ascii = score::ascii_text_score(sample.middle);
    notes.push(format!(
        "{} characters over {} sample pages, ascii text score {ascii:.2}",
        middle.len(),
        sample.middle_pages
    ));

    // Damage can only be judged where there is enough text to judge it, and
    // that guard comes first — including before the recovery attempt, which
    // on a short sample would otherwise answer for the whole document.
    if middle.len() >= limits.min_chars_to_judge {
        if reflect::undo(sample.middle).is_some() {
            notes.push("reflecting the character codes recovers ordinary text".into());
            return Triage {
                layer: TextLayer::ReflectedFont,
                needs_ocr: false,
                notes,
            };
        }
        let letters = score::letter_share(&middle);
        let latin1 = score::latin1_share(&middle);
        notes.push(format!("letters {letters:.2}, latin-1 {latin1:.2}"));
        if letters < DEAD_LETTER_SHARE || latin1 > DEAD_LATIN1_SHARE {
            return Triage {
                layer: TextLayer::Garbled,
                needs_ocr: true,
                notes,
            };
        }
    }

    if let Some(pages) = sample.pages
        && pages >= limits.min_pages_for_density
    {
        let density = middle.len() as f64 / sample.middle_pages.max(1) as f64;
        notes.push(format!("{density:.0} characters a page"));
        if density < SPARSE_CHARS_PER_PAGE {
            return Triage {
                layer: TextLayer::Sparse,
                needs_ocr: true,
                notes,
            };
        }
    }

    if let Some(share) = english_prose_share(sample.middle, limits.min_words) {
        notes.push(format!("english function words {share:.2}"));
        if share < HEALTHY_WORD_RATIO {
            notes.push("not English, or not prose — judged on shape alone".into());
        }
    }

    Triage {
        layer: TextLayer::Healthy,
        needs_ocr: false,
        notes,
    }
}

/// Share of words that are common English function words, or `None` when
/// there is too little text to judge.
///
/// English-only on purpose, and so never the whole verdict: it cannot judge
/// the German, Greek and Church Slavonic books that sit beside the English
/// ones. It is reported because when it does apply it is the most direct
/// evidence there is that a layer reads as prose.
pub fn english_prose_share(text: &str, min_words: usize) -> Option<f64> {
    let words: Vec<String> = text
        .split(textkit::is_space)
        .map(|word| word.trim_matches(WORD_TRIM).to_lowercase())
        .filter(|word| !word.is_empty())
        .collect();
    if words.len() < min_words {
        return None;
    }
    let common = words
        .iter()
        .filter(|word| COMMON_WORDS.binary_search(&word.as_str()).is_ok())
        .count();
    Some(common as f64 / words.len() as f64)
}

/// The pages to judge a layer by: the middle of the document.
///
/// A fixed window near the front is only ever the middle of a pamphlet. On a
/// real book it reads front matter, which is cleanly typeset boilerplate even
/// when the body is damaged — a 424-page commentary scored healthy on pages
/// 10 to 14 and garbled on both 160 to 164 and 300 to 304.
pub fn middle_window(pages: Option<u32>, span: u32) -> (u32, u32) {
    let Some(pages) = pages.filter(|pages| *pages >= 1) else {
        // The old window, for a document whose length is unknown.
        return (10, 14);
    };
    if pages <= span {
        return (1, pages);
    }
    let start = (pages / 2).saturating_sub(span / 2).max(1);
    (start, pages.min(start + span - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prose() -> String {
        "The second estate held its land in exchange for service, and the service was \
         military. That much is agreed; what follows is not. The court rolls do not say \
         how often the obligation was tested, because the clerk who recorded it had no \
         vocabulary for the difference between a rebellion and a bad harvest. This is the \
         central difficulty of the sources, and everything that follows depends on reading \
         around it rather than through it."
            .repeat(3)
    }

    fn sample<'a>(opening: &'a str, middle: &'a str, pages: Option<u32>) -> Sample<'a> {
        Sample {
            opening,
            middle,
            middle_pages: 5,
            pages,
        }
    }

    #[test]
    fn the_common_word_list_is_sorted_and_whole() {
        assert!(COMMON_WORDS.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(COMMON_WORDS.len(), 37, "a word has gone missing");
    }

    #[test]
    fn a_short_sample_is_judged_on_density_rather_than_damage() {
        // Too little text to call it damaged, which is what the density gate
        // is for. Answering from the recovery attempt instead would route the
        // file away from the OCR it needs.
        let stored: String = "The mill stood at the northern end. "
            .repeat(4)
            .chars()
            .map(|c| {
                let reflected = 0x120 - c as u32;
                char::from_u32(reflected)
                    .filter(|_| (0xa1..=0xff).contains(&reflected))
                    .unwrap_or(c)
            })
            .collect();
        let found = triage(&sample(&prose(), &stored, Some(400)), &Limits::default());
        assert_eq!(found.layer, TextLayer::Sparse, "{:#?}", found.notes);
        assert!(found.needs_ocr);
    }

    #[test]
    fn a_book_that_reads_is_healthy() {
        let text = prose();
        let found = triage(&sample(&text, &text, Some(400)), &Limits::default());
        assert_eq!(found.layer, TextLayer::Healthy);
        assert!(!found.needs_ocr);
    }

    #[test]
    fn a_scan_with_no_layer_is_absent() {
        let found = triage(&sample("  \n 12 \n", "", Some(400)), &Limits::default());
        assert_eq!(found.layer, TextLayer::Absent);
        assert!(found.needs_ocr);
    }

    #[test]
    fn a_layer_of_latin1_soup_is_garbled() {
        // Scrambled with some key other than the reflection, so that the
        // recovery pass declines it and the damage stands.
        let soup: String = (0..800)
            .map(|i: u32| char::from_u32(0xa1 + (i * 7) % 0x5f).expect("in the latin-1 band"))
            .collect();
        let opening = prose();
        let found = triage(&sample(&opening, &soup, Some(400)), &Limits::default());
        assert_eq!(found.layer, TextLayer::Garbled, "{:#?}", found.notes);
        assert!(found.needs_ocr);
    }

    #[test]
    fn a_layer_holding_only_running_heads_is_sparse() {
        let thin = "The Second Estate 41\n".repeat(8);
        let opening = prose();
        let found = triage(&sample(&opening, &thin, Some(400)), &Limits::default());
        assert_eq!(found.layer, TextLayer::Sparse, "{:#?}", found.notes);
        assert!(found.needs_ocr);
    }

    #[test]
    fn a_short_document_is_not_judged_on_density() {
        let thin = "Page 3\n".repeat(8);
        let found = triage(&sample(&prose(), &thin, Some(4)), &Limits::default());
        assert_ne!(found.layer, TextLayer::Sparse);
    }

    #[test]
    fn a_permuted_layer_is_recovered_rather_than_sent_for_ocr() {
        let stored: String = prose()
            .chars()
            .map(|c| {
                let reflected = 0x120 - c as u32;
                if (0xa1..=0xff).contains(&reflected) {
                    char::from_u32(reflected).expect("in range")
                } else {
                    c
                }
            })
            .collect();
        let found = triage(&sample(&stored, &stored, Some(400)), &Limits::default());
        assert_eq!(found.layer, TextLayer::ReflectedFont, "{:#?}", found.notes);
        assert!(!found.needs_ocr, "a recoverable layer must not be OCRed");
    }

    #[test]
    fn the_window_is_the_middle_of_the_book() {
        assert_eq!(middle_window(Some(400), 5), (198, 202));
        assert_eq!(middle_window(Some(3), 5), (1, 3));
        assert_eq!(middle_window(Some(5), 5), (1, 5));
        assert_eq!(middle_window(None, 5), (10, 14));
        assert_eq!(middle_window(Some(0), 5), (10, 14));
    }

    #[test]
    fn english_prose_is_recognised_and_soup_is_not() {
        let share = english_prose_share(&prose(), 80).expect("enough words");
        assert!(share > 0.25, "{share}");
        let soup = "xqz ".repeat(200);
        assert!(english_prose_share(&soup, 80).expect("enough words") < 0.01);
    }
}
