//! Digital object identifiers.
//!
//! A DOI is a registrant prefix beginning `10.` and a suffix the registrant
//! chooses, separated by a slash. The suffix may be almost anything, which is
//! what makes reading one out of running text hard: there is no character
//! that reliably ends it.

use std::sync::LazyLock;

use fancy_regex::Regex;

/// The shape, wherever it appears and however it was introduced.
///
/// The suffix stops at whitespace or at one of the brackets and quotes that
/// wrap a citation, because those are never part of one in practice and
/// leaving them on makes two spellings of the same identifier.
static DOI: LazyLock<Regex> = LazyLock::new(|| {
    // Brackets are part of a DOI far more often than they are punctuation
    // around one: Elsevier alone publishes hundreds of thousands in the form
    // `10.1016/S0140-6736(16)30370-1`. They are taken, and an unbalanced one
    // at the end — the closing half of a bracket the sentence opened — is
    // given back afterwards.
    Regex::new(r#"(?i)\b(10\.\d{4,9}/[^\s"'<>,;]+)"#).expect("a valid pattern")
});

/// The prefixes a DOI arrives wearing.
const PREFIXES: &[&str] = &[
    "https://doi.org/",
    "http://doi.org/",
    "https://dx.doi.org/",
    "http://dx.doi.org/",
    "doi:",
    "doi ",
];

/// Read a DOI out of a string, in the form it should be stored in.
///
/// Lowercased, because a DOI is case-insensitive and two catalogues will
/// disagree about the case of the same one. The trailing punctuation a
/// sentence leaves behind is removed.
pub fn parse(text: &str) -> Option<String> {
    let text = text.trim();
    let mut without = text;
    for prefix in PREFIXES {
        // `get` rather than a slice: the prefixes are ASCII and the text is
        // not, so cutting at the prefix's byte length can land inside a
        // character. A DOI copied out of a PDF arrives full of zero-width
        // spaces, and `10.1093/\u{200b}oso/` is exactly long enough to put
        // one where `https://doi.org/` ends.
        if without
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            without = &without[prefix.len()..];
            break;
        }
    }

    let found = DOI.captures(without).ok().flatten()?;
    let id = found.get(1)?.as_str();

    // A full stop, comma or semicolon at the end belongs to the sentence
    // rather than to the identifier. A closing bracket does too, unless the
    // suffix opened one.
    let id = id.trim_end_matches(['.', ',', ';', ':']);
    let id = trim_unclosed(id);
    let id = id.trim_end_matches(['.', ',', ';', ':']);
    if id.len() < 6 || !id.contains('/') {
        return None;
    }
    // A suffix of nothing is not a suffix.
    if id.split_once('/')?.1.is_empty() {
        return None;
    }
    Some(id.to_lowercase())
}

/// Give back a closing bracket the DOI never opened.
///
/// `(10.1234/foo)` in a sentence ends on a bracket that belongs to the
/// sentence; `10.1016/S0140-6736(16)30370-1` ends on digits and keeps the
/// pair in the middle of it. Counting is what tells them apart.
fn trim_unclosed(id: &str) -> &str {
    let mut end = id.len();
    loop {
        let head = &id[..end];
        let Some(last) = head.chars().last() else {
            return head;
        };
        let opener = match last {
            ')' => '(',
            ']' => '[',
            '}' => '{',
            _ => return head,
        };
        let body = &head[..head.len() - last.len_utf8()];
        if body.matches(opener).count() > body.matches(last).count() {
            return head;
        }
        end -= last.len_utf8();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_doi_may_have_brackets_in_it() {
        // Elsevier publishes hundreds of thousands in this form, and the
        // pattern used to stop at the first bracket and hand back
        // `10.1016/s0140-6736` — a different, wrong, resolvable DOI.
        assert_eq!(
            parse("10.1016/S0140-6736(16)30370-1").as_deref(),
            Some("10.1016/s0140-6736(16)30370-1")
        );
        assert_eq!(
            parse("https://doi.org/10.1016/S0140-6736(23)00441-5").as_deref(),
            Some("10.1016/s0140-6736(23)00441-5")
        );
        assert_eq!(parse("10.1234/a[b]c").as_deref(), Some("10.1234/a[b]c"));
    }

    #[test]
    fn a_bracket_the_sentence_opened_is_given_back() {
        for (text, want) in [
            ("see (10.1234/foo) here", "10.1234/foo"),
            ("10.1234/foo).", "10.1234/foo"),
            ("[10.1234/foo]", "10.1234/foo"),
            ("10.1234/foo(bar)", "10.1234/foo(bar)"),
        ] {
            assert_eq!(parse(text).as_deref(), Some(want), "{text}");
        }
    }

    use super::*;

    #[test]
    fn the_plain_form_reads_as_itself() {
        assert_eq!(parse("10.1000/182").as_deref(), Some("10.1000/182"));
    }

    #[test]
    fn every_way_of_introducing_one_gives_the_same_identifier() {
        for text in [
            "doi:10.1000/182",
            "DOI: 10.1000/182",
            "https://doi.org/10.1000/182",
            "http://dx.doi.org/10.1000/182",
            "HTTPS://DOI.ORG/10.1000/182",
        ] {
            assert_eq!(parse(text).as_deref(), Some("10.1000/182"), "{text}");
        }
    }

    #[test]
    fn the_sentence_around_it_is_not_part_of_it() {
        assert_eq!(
            parse("See 10.1093/mind/LIX.236.433, which argues otherwise.").as_deref(),
            Some("10.1093/mind/lix.236.433")
        );
        assert_eq!(parse("(10.1000/182)").as_deref(), Some("10.1000/182"));
        assert_eq!(parse("\"10.1000/182\"").as_deref(), Some("10.1000/182"));
    }

    #[test]
    fn case_is_not_part_of_it_either() {
        // A DOI is case-insensitive, and two catalogues will disagree.
        assert_eq!(
            parse("10.1093/MIND/LIX.236.433").as_deref(),
            parse("10.1093/mind/lix.236.433").as_deref()
        );
    }

    #[test]
    fn a_suffix_may_hold_almost_anything() {
        assert_eq!(
            parse("10.1371/journal.pone.0012345").as_deref(),
            Some("10.1371/journal.pone.0012345")
        );
        assert_eq!(
            parse("10.1016/j.cell.2020.01.001").as_deref(),
            Some("10.1016/j.cell.2020.01.001")
        );
    }

    #[test]
    fn what_is_not_a_doi_is_not_one() {
        for text in ["", "10.", "10.1000", "11.1000/182", "10.100/182", "hello"] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_prefix_with_no_identifier_after_it_is_nothing() {
        assert_eq!(parse("doi:"), None);
        assert_eq!(parse("https://doi.org/"), None);
        assert_eq!(parse("10.1000/"), None);
    }
}
