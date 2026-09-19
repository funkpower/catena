//! Turning a record into a readable identifier, and judging the one it has.

use std::sync::LazyLock;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::entities4::NAMED;

/// Apostrophes. The ones written in a title become a separator; the ones
/// transliteration produces afterwards — a Cyrillic hard sign becomes one —
/// are dropped, so they do not each turn into a dash of their own.
static QUOTES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[']+").expect("valid"));

/// A named character reference.
static NAMED_REFERENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&([a-zA-Z][a-zA-Z0-9]*);").expect("valid"));

static DECIMAL_REFERENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&#(\d+);").expect("valid"));

static HEX_REFERENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&#x([\da-fA-F]+);").expect("valid"));

/// A comma inside a number, which is a thousands separator rather than a word
/// break: `12,000` is one number.
static THOUSANDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d),(\d)").expect("valid"));

/// Everything a slug may not contain.
static DISALLOWED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^-a-zA-Z0-9]+").expect("valid"));

static DASH_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-{2,}").expect("valid"));

/// How many characters a whole slug may take.
pub const MAX_LENGTH: usize = 80;

/// How many of them the lead author may take.
///
/// The title always needs room. Measured against a review queue: a
/// five-editor volume otherwise consumed the whole budget and left a slug
/// naming only people, which cannot be recognised as a book.
pub const AUTHOR_BUDGET: usize = 30;

/// Reduce a text to a slug.
///
/// Transliterated to ASCII, lowercased, everything else turned into a
/// separator, and truncated at a word if a maximum is given.
pub fn slugify(text: &str, max_length: usize, word_boundary: bool) -> String {
    let text = QUOTES.replace_all(text, "-");
    let text: String = deunicode::deunicode(&text.nfkd().collect::<String>());

    let text = NAMED_REFERENCE.replace_all(&text, |found: &regex::Captures| {
        match NAMED.binary_search_by_key(&&found[1], |(name, _)| name) {
            Ok(at) => NAMED[at].1.to_string(),
            Err(_) => found[0].to_string(),
        }
    });
    let text = DECIMAL_REFERENCE.replace_all(&text, |found: &regex::Captures| {
        character(found[1].parse::<u32>().ok(), &found[0])
    });
    let text = HEX_REFERENCE.replace_all(&text, |found: &regex::Captures| {
        character(u32::from_str_radix(&found[1], 16).ok(), &found[0])
    });

    let text: String = text.nfkd().collect::<String>().to_lowercase();
    let text = QUOTES.replace_all(&text, "");
    let text = THOUSANDS.replace_all(&text, "$1$2");
    let text = DISALLOWED.replace_all(&text, "-");
    let text = DASH_RUN.replace_all(&text, "-");
    let text = text.trim_matches('-');

    match max_length {
        0 => text.to_string(),
        limit => truncate(text, limit, word_boundary),
    }
}

fn character(code: Option<u32>, whole: &str) -> String {
    code.and_then(char::from_u32)
        .map_or_else(|| whole.to_string(), String::from)
}

/// Cut a slug to `limit` characters.
///
/// At a word, when asked — and a word that does not fit is skipped rather
/// than ending the slug, so a long word in the middle of a title does not
/// cost every word after it. `an-introduction-to-historical-analysis` cut to
/// twenty-eight is `an-introduction-to-analysis`.
fn truncate(text: &str, limit: usize, word_boundary: bool) -> String {
    if text.len() < limit {
        return text.to_string();
    }
    if !word_boundary {
        return text[..floor_boundary(text, limit)]
            .trim_matches('-')
            .to_string();
    }
    if !text.contains('-') {
        return text[..floor_boundary(text, limit)].to_string();
    }

    let mut kept = String::new();
    for word in text.split('-') {
        if word.is_empty() {
            continue;
        }
        let would_be = kept.len() + word.len();
        if would_be < limit {
            kept.push_str(word);
            kept.push('-');
        } else if would_be == limit {
            kept.push_str(word);
            break;
        }
    }
    if kept.is_empty() {
        kept = text[..floor_boundary(text, limit)].to_string();
    }
    kept.trim_matches('-').to_string()
}

/// The largest character boundary at or below `at`, so a cut never lands
/// inside a character.
fn floor_boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// The first name in an author field, which may list several.
///
/// An edited volume can carry five names, and the whole list ahead of the
/// title spends the entire budget on people. An identifier needs one name and
/// the work; the rest are in the record.
pub fn lead_author(author: &str) -> String {
    static SEPARATOR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\s*[&;]\s*|\s+and\s+").expect("valid"));

    let author = author.trim_matches(textkit::is_space);
    let lead = SEPARATOR.splitn(author, 2).next().unwrap_or("");
    // Commas are ambiguous: "Bishop, Christopher M." is one person, but
    // "Miranda Lundy, Daud Sutton, Anthony Ashton" is three — and taking the
    // whole list truncates it mid-name at the budget. Two or more commas
    // means a list; one means a surname first.
    let lead = match lead.matches(',').count() >= 2 {
        true => lead.split_once(',').map_or(lead, |(first, _)| first),
        false => lead,
    };
    lead.trim_matches(|c: char| textkit::is_space(c) || c == ',')
        .to_string()
}

/// The slug a record's current fields imply.
///
/// The author is reduced to its lead name and given a bounded share of the
/// budget, so the title is always represented.
pub fn slug_for(author: Option<&str>, title: Option<&str>, id: i64) -> String {
    let lead = slugify(&lead_author(author.unwrap_or("")), AUTHOR_BUDGET, true);
    let room = MAX_LENGTH.saturating_sub(if lead.is_empty() { 0 } else { lead.len() + 1 });
    let work = slugify(title.unwrap_or(""), room.max(20), true);

    let base = [lead.as_str(), work.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    match base.is_empty() {
        true => format!("book-{id}"),
        false => format!("{base}-{id}"),
    }
}

/// Head tokens that mean the slug was minted without an author.
///
/// The slug's first token comes from the author field, so a placeholder there
/// is direct evidence the slug was made blind — independent of whatever the
/// record says now.
const PLACEHOLDER_AUTHORS: &[&str] = &[
    "admin",
    "administrator",
    "anonymous",
    "book",
    "default",
    "guest",
    "imac",
    "na",
    "none",
    "someauthor",
    "test",
    "unknown",
    "untitled",
    "user",
];

/// Words that carry no weight when deciding whether a slug and a record are
/// describing the same book.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "by", "de", "for", "from", "in", "is", "la", "le", "of", "on",
    "or", "the", "to", "with",
];

/// Below this share of the slug's meaningful tokens appearing in the current
/// title or author, the slug is describing some other book.
///
/// Deliberately low: above it sit ordinary drifts — a subtitle dropped, an
/// editor removed — and a disagreement this size is held for review either
/// way.
const WRONG_BOOK_PRECISION: f64 = 0.34;

/// A new slug that recovers less than this much of the current title is not
/// clearly an improvement.
const MIN_TITLE_RECALL: f64 = 0.35;

/// Traces of the file a book arrived as, rather than of the book: mirror
/// suffixes, a wrapper format, a word processor's name, a Windows path.
static ARTEFACT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"z-lib|1lib|zlibrary|djvu|microsoft-word|documents-and-settings|(?:^|-)(?:pdf|doc|docx|txt|tmp|epub|mobi|azw3|rtf|prnt|qxd|qxp|fm)(?:-|$)",
    )
    .expect("a valid pattern")
});

/// A volume marker.
///
/// One in the current slug is worth more than a clean slug without: several
/// volumes of a proceedings share a title, and their existing slugs are what
/// tell them apart.
static VOLUME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|-)(?:vol|volume|part|book|bk|no|nr|tome|band)-?(?:\d+|[ivxl]+)(?:-|$)")
        .expect("a valid pattern")
});

/// An author field that has swallowed part of the title or the series.
///
/// A slug built from that names nothing, and it is junk of a kind the
/// placeholder and artefact tests cannot see, because the text is neither a
/// handle nor a filename.
static SPILLED_AUTHOR: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
    use textkit::pattern::{WORD_END, WORD_START};
    fancy_regex::Regex::new(&format!(
        r"(?i)[()\[\]]|{WORD_START}volume{WORD_END}|{WORD_START}an introduction{WORD_END}"
    ))
    .expect("a valid pattern")
});

/// Tiers that rest on evidence inside the slug itself, and so can be applied
/// without a person reading them.
///
/// A placeholder author or a filename artefact is a property of the slug and
/// cannot be wrong about itself. Everything else is a *disagreement* between
/// the slug and the record, and a disagreement does not say which side is at
/// fault.
pub const SAFE_TIERS: &[&str] = &["filename-artefact", "placeholder-author"];

/// What should happen to a record's slug.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Proposal {
    /// The slug the record has.
    pub current: String,
    /// The slug its fields imply.
    pub proposed: String,
    /// How much the evidence is worth; see [`SAFE_TIERS`].
    pub tier: String,
    /// Why, in the order the evidence was found.
    pub reasons: Vec<String>,
}

impl Proposal {
    /// Whether this can be applied without a person reading it.
    pub fn is_safe(&self) -> bool {
        SAFE_TIERS.binary_search(&self.tier.as_str()).is_ok()
    }
}

/// Judge a record's slug. `None` when there is nothing wrong with it.
///
/// What comes back is a tier rather than a verdict, because the three kinds
/// of evidence differ in kind and only two of them are about the slug alone.
pub fn evaluate(
    current: &str,
    author: Option<&str>,
    title: Option<&str>,
    id: i64,
) -> Option<Proposal> {
    let body = strip_id(current);
    let body_tokens = tokens(body);
    let title_tokens: Vec<String> = tokens(&slugify(title.unwrap_or(""), 0, false))
        .into_iter()
        .map(str::to_string)
        .collect();
    let author_tokens: Vec<String> = tokens(&slugify(author.unwrap_or(""), 0, false))
        .into_iter()
        .map(str::to_string)
        .collect();

    let meaningful: Vec<&str> = body_tokens
        .iter()
        .copied()
        .filter(|token| STOPWORDS.binary_search(token).is_err() && token.chars().count() > 1)
        .collect();
    let precision = match meaningful.is_empty() {
        true => 0.0,
        false => {
            let matched = meaningful
                .iter()
                .filter(|token| {
                    title_tokens.iter().any(|t| t == *token)
                        || author_tokens.iter().any(|t| t == *token)
                })
                .count();
            matched as f64 / meaningful.len() as f64
        }
    };

    let author_text = author.unwrap_or("").trim_matches(textkit::is_space);
    let author_known = !author_text.is_empty() && author.unwrap_or("") != "Unknown";

    let mut reasons = Vec::new();
    let tier = if body_tokens
        .first()
        .is_some_and(|head| PLACEHOLDER_AUTHORS.binary_search(head).is_ok())
        && author_known
    {
        reasons.push(format!(
            "slug opens with placeholder author '{}'",
            body_tokens[0]
        ));
        "placeholder-author"
    } else if ARTEFACT.is_match(body) {
        reasons.push("slug carries a filename or mirror artefact".to_string());
        "filename-artefact"
    } else if precision < WRONG_BOOK_PRECISION {
        reasons.push(format!(
            "only {} of slug tokens appear in the current title/author — slug and metadata disagree",
            textkit::percent(precision)
        ));
        "wrong-book"
    } else {
        return None;
    };

    let proposed = slug_for(author, title, id);
    let proposed_body = strip_id(&proposed);
    let proposed_tokens = tokens(proposed_body);

    if proposed == current {
        reasons.push("recomputes to the same slug: fix the metadata first".into());
        return Some(Proposal {
            current: current.into(),
            proposed,
            tier: "noop-metadata-junk".into(),
            reasons,
        });
    }

    let recall = |slug_tokens: &[&str]| -> f64 {
        match title_tokens.is_empty() {
            true => 0.0,
            false => {
                let found = title_tokens
                    .iter()
                    .filter(|token| slug_tokens.iter().any(|t| t == token))
                    .count();
                found as f64 / title_tokens.len() as f64
            }
        }
    };
    let proposed_recall = recall(&proposed_tokens);
    let recovers_title = proposed_recall > recall(&body_tokens);

    // Is the proposal better than what is there, even if imperfect? A slug
    // still saying "unknown" because the author genuinely is, but now naming
    // the book, beats one naming a temporary directory. So the test is not
    // "is the new slug clean" but "does it recover title the current one
    // lacks, without carrying an artefact of its own".
    let still_placeholder = proposed_tokens
        .first()
        .is_some_and(|head| PLACEHOLDER_AUTHORS.binary_search(head).is_ok());
    if ARTEFACT.is_match(proposed_body) || (still_placeholder && !recovers_title) {
        reasons.push("new slug is no better: fix the metadata first".into());
        return Some(Proposal {
            current: current.into(),
            proposed,
            tier: "defer-new-slug-junk".into(),
            reasons,
        });
    }

    // Only when the author actually reaches the slug: an author field of
    // nothing but brackets slugifies to nothing, so the proposal leads with
    // the title and is fine.
    let lead_in_slug = slugify(&lead_author(author_text), AUTHOR_BUDGET, true);
    if !lead_in_slug.is_empty() && SPILLED_AUTHOR.is_match(author_text).unwrap_or(false) {
        reasons.push(format!(
            "author field looks like spilled title/series text: '{author_text}'"
        ));
        return Some(Proposal {
            current: current.into(),
            proposed,
            tier: "defer-new-slug-junk".into(),
            reasons,
        });
    }

    if VOLUME.is_match(body) && !VOLUME.is_match(proposed_body) {
        reasons.push("current slug carries a volume marker the new one loses".into());
        return Some(Proposal {
            current: current.into(),
            proposed,
            tier: "review-loses-volume".into(),
            reasons,
        });
    }

    // Measured on the proposal itself rather than recomputed from the fields:
    // the generator budgets the author separately, so a formula that
    // re-derives the length describes a slug it no longer builds.
    let truncated = proposed_body.len() >= MAX_LENGTH - 1;
    if truncated || proposed_recall < MIN_TITLE_RECALL {
        reasons.push(format!(
            "new slug recovers only {} of the title",
            textkit::percent(proposed_recall)
        ));
        return Some(Proposal {
            current: current.into(),
            proposed,
            tier: "review-low-recall".into(),
            reasons,
        });
    }

    Some(Proposal {
        current: current.into(),
        proposed,
        tier: tier.into(),
        reasons,
    })
}

/// A slug without its trailing numeric id.
fn strip_id(slug: &str) -> &str {
    static TRAILING_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-\d+$").expect("valid"));
    match TRAILING_ID.find(slug) {
        Some(found) if found.end() == slug.len() => &slug[..found.start()],
        _ => slug,
    }
}

fn tokens(slug: &str) -> Vec<&str> {
    slug.split('-').filter(|token| !token.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entity_table_is_sorted_so_it_can_be_searched() {
        assert!(NAMED.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn ordinary_text_slugifies() {
        assert_eq!(slugify("William Zinsser", 0, false), "william-zinsser");
        assert_eq!(
            slugify("Hello---World  &  Friends", 0, false),
            "hello-world-friends"
        );
        assert_eq!(slugify("", 0, false), "");
    }

    #[test]
    fn a_written_apostrophe_is_a_word_break() {
        assert_eq!(
            slugify("Kierkegaard's Conception", 0, false),
            "kierkegaard-s-conception"
        );
    }

    #[test]
    fn an_apostrophe_transliteration_produced_is_not() {
        // The Cyrillic hard sign becomes one, and a dash for each would make
        // a slug of fragments.
        assert_eq!(slugify("Олегъ", 0, false), "oleg");
    }

    #[test]
    fn other_scripts_are_transliterated() {
        assert_eq!(slugify("Être et Temps", 0, false), "etre-et-temps");
        assert_eq!(slugify("café naïve", 0, false), "cafe-naive");
        assert_eq!(slugify("Straße", 0, false), "strasse");
        assert_eq!(slugify("Ἀθῆναι καὶ Μόσχα", 0, false), "athenai-kai-moskha");
    }

    #[test]
    fn a_thousands_separator_does_not_split_a_number() {
        assert_eq!(slugify("12,000 Years", 0, false), "12000-years");
    }

    #[test]
    fn character_references_are_decoded() {
        assert_eq!(slugify("Tom &amp; Jerry", 0, false), "tom-jerry");
        assert_eq!(slugify("caf&#233;", 0, false), "cafe");
        assert_eq!(slugify("caf&#xe9;", 0, false), "cafe");
        assert_eq!(slugify("&notaname; here", 0, false), "notaname-here");
    }

    #[test]
    fn a_cut_lands_on_a_word() {
        assert_eq!(
            slugify("An Introduction to Historical Analysis", 28, true),
            "an-introduction-to-analysis"
        );
        assert_eq!(
            slugify("An Introduction to Historical Analysis", 28, false),
            "an-introduction-to-historica"
        );
    }

    #[test]
    fn a_lead_author_is_one_person() {
        assert_eq!(lead_author("A. A. Long & J. A. Mansfeld"), "A. A. Long");
        assert_eq!(
            lead_author("Clarke, Arthur C. and Lee, Gentry"),
            "Clarke, Arthur C."
        );
        assert_eq!(
            lead_author("Bishop, Christopher M."),
            "Bishop, Christopher M."
        );
        assert_eq!(
            lead_author("Miranda Lundy, Daud Sutton, Anthony Ashton"),
            "Miranda Lundy"
        );
        assert_eq!(lead_author(""), "");
    }

    #[test]
    fn a_slug_names_a_person_and_a_work() {
        assert_eq!(
            slug_for(Some("William Zinsser"), Some("On Writing Well"), 3450),
            "william-zinsser-on-writing-well-3450"
        );
    }

    #[test]
    fn a_slug_always_has_room_for_the_title() {
        let slug = slug_for(
            Some("A. A. Long & J. A. Mansfeld & Keimpe Algra & Carl A. Huffman"),
            Some("The Cambridge Companion to Early Greek Philosophy"),
            1234,
        );
        assert!(slug.contains("cambridge"), "{slug}");
        assert!(slug.len() <= MAX_LENGTH + 6, "{slug}");
    }

    #[test]
    fn a_record_with_nothing_usable_still_gets_a_slug() {
        assert_eq!(slug_for(Some("()"), Some(""), 7023), "book-7023");
        assert_eq!(slug_for(None, None, 1), "book-1");
    }

    #[test]
    fn a_slug_that_agrees_with_its_record_is_left_alone() {
        assert_eq!(
            evaluate(
                "william-zinsser-on-writing-well-3450",
                Some("William Zinsser"),
                Some("On Writing Well"),
                3450
            ),
            None
        );
    }

    #[test]
    fn a_slug_minted_without_an_author_is_safe_to_replace() {
        let found = evaluate(
            "unknown-mind-and-brain-1234",
            Some("Zinsser, William"),
            Some("Mind and Brain"),
            1234,
        )
        .expect("a proposal");
        assert_eq!(found.tier, "placeholder-author");
        assert!(found.is_safe());
        assert_eq!(found.proposed, "zinsser-william-mind-and-brain-1234");
    }

    #[test]
    fn a_slug_carrying_a_filename_artefact_is_safe_to_replace() {
        let found = evaluate(
            "smith-john-the-mill-z-lib-org-99",
            Some("Smith, John"),
            Some("The Mill and the Valley"),
            99,
        )
        .expect("a proposal");
        assert_eq!(found.tier, "filename-artefact");
        assert!(found.is_safe());
    }

    #[test]
    fn a_slug_naming_another_book_is_held_for_review() {
        let found = evaluate(
            "william-zinsser-on-writing-well-3450",
            Some("Laura Nader"),
            Some("Weaponizing Anthropology"),
            3450,
        )
        .expect("a proposal");
        assert_eq!(found.tier, "wrong-book");
        assert!(
            !found.is_safe(),
            "a disagreement does not say which side is wrong"
        );
    }

    #[test]
    fn a_proposal_that_changes_nothing_says_to_fix_the_record() {
        // The record's own fields carry the artefact, so regenerating from
        // them reproduces it exactly.
        let found = evaluate(
            "microsoft-word-mat-doc-1",
            Some("Microsoft Word"),
            Some("mat doc"),
            1,
        )
        .expect("a proposal");
        assert_eq!(found.tier, "noop-metadata-junk");
        assert_eq!(found.proposed, found.current);
    }

    #[test]
    fn a_new_slug_that_is_still_a_placeholder_is_deferred() {
        let found =
            evaluate("unknown-1234", Some("Unknown Author"), None, 1234).expect("a proposal");
        assert_eq!(found.tier, "defer-new-slug-junk");
    }

    #[test]
    fn a_volume_marker_the_new_slug_would_lose_is_held_for_review() {
        let found = evaluate(
            "unknown-proceedings-of-the-society-vol-9-77",
            Some("Aristotelian Society"),
            Some("Proceedings of the Society"),
            77,
        )
        .expect("a proposal");
        assert_eq!(found.tier, "review-loses-volume");
    }

    #[test]
    fn an_author_field_holding_title_text_is_not_used_to_rename() {
        let found = evaluate(
            "unknown-the-land-of-iran-55",
            Some("the land of iran (Cambridge University Press"),
            Some("The Land of Iran"),
            55,
        )
        .expect("a proposal");
        assert_eq!(found.tier, "defer-new-slug-junk");
    }

    #[test]
    fn an_author_that_slugifies_to_nothing_leaves_the_title_leading() {
        assert_eq!(
            slug_for(Some("()"), Some("Mind and Brain"), 7023),
            "mind-and-brain-7023"
        );
    }
}
