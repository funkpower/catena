//! arXiv preprint identifiers.
//!
//! Two forms, because the scheme changed in 2007. The new one is four digits
//! of year and month, a dot, and a number: `2401.01234`. The old one is an
//! archive name, optionally a subject class, a slash, and seven digits:
//! `math.GT/0309136`.
//!
//! Either may carry a version — `v2` — which is part of the identifier: it
//! names a particular revision, and citing one without it means the latest.

use std::sync::LazyLock;

use fancy_regex::Regex;

/// The post-2007 form: `YYMM.NNNNN`, four or five digits after the dot.
static MODERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\d{4}\.\d{4,5}(?:v\d+)?)\b").expect("a valid pattern"));

/// The pre-2007 form: an archive, an optional subject class, then seven
/// digits.
static LEGACY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z][a-z-]{1,20}(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?)\b")
        .expect("a valid pattern")
});

/// Read an arXiv identifier out of a string.
///
/// The old form keeps the case of its subject class, which is capitalised by
/// convention and is how the archives are written; the rest is lowercased.
pub fn parse(text: &str) -> Option<String> {
    let text = text.trim();
    let without = strip_prefix(text);

    if let Some(found) = LEGACY.captures(without).ok().flatten() {
        let id = found.get(1)?.as_str();
        let (archive, number) = id.split_once('/')?;
        return Some(format!(
            "{}/{}",
            lowercase_archive(archive),
            number.to_lowercase()
        ));
    }
    let found = MODERN.captures(without).ok().flatten()?;
    Some(found.get(1)?.as_str().to_lowercase())
}

/// The archive lowercased, but its subject class left as it was written.
fn lowercase_archive(archive: &str) -> String {
    match archive.split_once('.') {
        Some((name, class)) => format!("{}.{}", name.to_lowercase(), class.to_uppercase()),
        None => archive.to_lowercase(),
    }
}

/// Whatever introduced it.
fn strip_prefix(text: &str) -> &str {
    for prefix in [
        "https://arxiv.org/abs/",
        "http://arxiv.org/abs/",
        "arxiv:",
        "arxiv ",
    ] {
        // See the note in `doi::parse`: a slice at the prefix's byte length
        // can land inside a character the text happens to carry.
        if text
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            return text[prefix.len()..].trim();
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modern_form_reads_as_itself() {
        assert_eq!(parse("2401.01234").as_deref(), Some("2401.01234"));
        // Four digits after the dot before 2015, five after it.
        assert_eq!(parse("0704.0001").as_deref(), Some("0704.0001"));
    }

    #[test]
    fn a_version_is_part_of_the_identifier() {
        // It names a revision; without it a citation means the latest.
        assert_eq!(parse("2401.01234v2").as_deref(), Some("2401.01234v2"));
        assert_eq!(
            parse("math.GT/0309136v1").as_deref(),
            Some("math.GT/0309136v1")
        );
    }

    #[test]
    fn the_old_form_keeps_its_subject_class_capitalised() {
        assert_eq!(parse("math.GT/0309136").as_deref(), Some("math.GT/0309136"));
        assert_eq!(parse("MATH.gt/0309136").as_deref(), Some("math.GT/0309136"));
        assert_eq!(parse("hep-th/9901001").as_deref(), Some("hep-th/9901001"));
    }

    #[test]
    fn every_way_of_introducing_one_gives_the_same_identifier() {
        for text in [
            "arXiv:2401.01234",
            "arxiv:2401.01234",
            "arXiv 2401.01234",
            "https://arxiv.org/abs/2401.01234",
        ] {
            assert_eq!(parse(text).as_deref(), Some("2401.01234"), "{text}");
        }
    }

    #[test]
    fn what_is_not_an_arxiv_identifier_is_not_one() {
        for text in ["", "hello", "24.01234", "2401.012", "2401.012345"] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_doi_is_not_mistaken_for_one() {
        // `10.1000/182` has a slash and digits, but not seven of them.
        assert_eq!(parse("10.1000/182"), None);
    }
}
