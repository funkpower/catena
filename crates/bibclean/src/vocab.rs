//! Finding the near-duplicates in a catalogue's vocabulary.
//!
//! An author, a publisher or a subject heading accumulates spellings: the
//! same person entered surname-first once and given-name-first twice, a
//! publisher with and without its legal suffix, a subject with a stray
//! bracket. Each spelling is a separate heading, so the books under one are
//! invisible from the others.
//!
//! What comes out is a proposal per alias, never a merge. Two headings that
//! normalise to the same string are usually the same thing and occasionally
//! are not, and a catalogue that has merged two real people is harder to
//! repair than one that never did.

use std::collections::HashMap;
use std::sync::LazyLock;

use fancy_regex::Regex as Boundaried;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use textkit::pattern::{WORD_END, WORD_START};
use unicode_normalization::UnicodeNormalization;

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

fn bounded(source: &str) -> Boundaried {
    Boundaried::new(source).expect("a valid pattern")
}

/// Suffixes that say what a company is rather than who it is.
static LEGAL_SUFFIX: LazyLock<Boundaried> = LazyLock::new(|| {
    bounded(&format!(
        r"(?i){WORD_START}(inc|ltd|llc|gmbh|co|company|limited|usa|uk){WORD_END}"
    ))
});

/// Everything in a publisher's name that is not the publisher.
static PUBLISHER_NOISE: LazyLock<Boundaried> = LazyLock::new(|| {
    bounded(&format!(
        r"(?i){WORD_START}(inc|ltd|llc|gmbh|co|company|limited|usa|uk|press|publishing|publishers|books|media|group|university){WORD_END}"
    ))
});

/// A role attached to a name, which is not part of the name.
static AUTHOR_ROLE: LazyLock<Boundaried> = LazyLock::new(|| {
    bounded(&format!(
        r"(?i){WORD_START}(editors?|eds?|translator|trans|compiler|author){WORD_END}"
    ))
});

/// Headings that are a file format or a watermark rather than a subject.
static FORMAT_OR_WATERMARK: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)^(z-?lib(\.org)?|libgen|annas?-?archive|ocr|untitled( document)?|full page fax print|tam sayfa faks|microsoft word|ebook|pdf|epub|mobi|azw3?|kindle|unknown|n/?a|none|untagged|#genre|#)$",
    )
});

/// A catalogue field that was exploded into the subject list.
static CATALOGUE_FIELD: LazyLock<Boundaried> = LazyLock::new(|| {
    bounded(&format!(
        r"(?i)^(#\s*)?(isbn([-\s]?1[03])?|asin|publication date|number of pages|publisher|genre){WORD_END}"
    ))
});

static ISBN_IN_TAG: LazyLock<Boundaried> = LazyLock::new(|| {
    bounded(&format!(
        r"(?:97[89][\s\-–—]?)?(?:[0-9][\s\-–—]?){{9}}[0-9Xx]{WORD_END}"
    ))
});

static URL: LazyLock<Regex> = LazyLock::new(|| pattern(r"(?i)https?://"));
static YEAR_ONLY: LazyLock<Regex> = LazyLock::new(|| pattern(r"^\d{4}\.?$"));
static ISO_DATE: LazyLock<Regex> = LazyLock::new(|| pattern(r"^\d{4}-\d{2}-\d{2}"));
static BARE_NUMBER: LazyLock<Regex> = LazyLock::new(|| pattern(r"^\d{1,6}$"));
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| pattern(r"^\d{1,2}:\d{2}(:\d{2})?"));
static PUNCTUATION_ONLY: LazyLock<Regex> = LazyLock::new(|| pattern(r"^[\W_]+$"));
static SPACE_RUN: LazyLock<Regex> = LazyLock::new(|| pattern(r"\s+"));
static NOT_ALPHANUMERIC: LazyLock<Regex> = LazyLock::new(|| pattern(r"[^a-z0-9]+"));
static INITIALS_TOGETHER: LazyLock<Regex> = LazyLock::new(|| pattern(r"[A-Z]\.[A-Z]"));
static INITIALS_SPACED: LazyLock<Regex> = LazyLock::new(|| pattern(r"[A-Z]\.\s[A-Z]"));

/// Suffixes on a person's name that are not part of who they are.
const NAME_SUFFIXES: &[&str] = &["ii", "iii", "iv", "jr", "md", "phd", "sr"];

/// Device names that an e-reader stamps onto every book it touches.
///
/// The generic ones only. A device named after its owner is a fact about one
/// collection, which is what `Rules` is for.
pub const DEVICE_PREFIXES: &[&str] = &["internet", "kindle", "kobo"];

/// A heading a catalogue holds, and how many works are under it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Item {
    /// The catalogue's own identifier for the heading.
    pub id: i64,
    /// The heading, as written.
    pub name: String,
    /// How many works are filed under it.
    #[serde(default)]
    pub count: i64,
}

/// What kind of heading a list holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A person who wrote something.
    Author,
    /// A publisher or an imprint.
    Publisher,
    /// A subject heading.
    Tag,
}

impl Kind {
    /// What this kind is called.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Author => "author",
            Kind::Publisher => "publisher",
            Kind::Tag => "tag",
        }
    }
}

/// What should happen to one heading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Proposal {
    /// Which kind of heading this is.
    pub kind: String,
    /// `merge` or `delete`.
    pub action: String,
    /// The heading to keep, when there is one.
    pub keep: Option<String>,
    /// The heading to lose.
    pub drop: String,
    /// The kept heading's identifier.
    pub keep_id: Option<i64>,
    /// The lost heading's identifier.
    pub drop_id: i64,
    /// How many works are under the kept heading.
    pub keep_count: i64,
    /// How many are under the lost one.
    pub drop_count: i64,
    /// Which rule produced this.
    pub method: String,
    /// How far to trust it, from zero to one.
    #[serde(with = "corpus_core::finite")]
    #[schemars(with = "f64")]
    pub confidence: f64,
    /// What a person needs to see to agree with it.
    pub evidence: String,
}

/// Collection-specific vocabulary, supplied rather than compiled in.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rules {
    /// Extra device prefixes, beyond the generic ones, that mark a heading
    /// as an e-reader's stamp rather than a subject.
    ///
    /// A device named after its owner belongs here and not in the program.
    #[serde(default)]
    pub device_prefixes: Vec<String>,
}

/// Every proposal a list of headings suggests.
///
/// Groups of near-duplicates first, then — for subjects — the headings that
/// are not subjects at all and should simply go.
pub fn cluster(kind: Kind, items: &[Item], rules: &Rules) -> Vec<Proposal> {
    let mut out = Vec::new();
    let mut grouped: Vec<i64> = Vec::new();

    for (method, group) in groups(kind, items) {
        out.extend(proposals_from_group(kind, &group, method));
        grouped.extend(group.iter().map(|item| item.id));
    }
    if kind == Kind::Tag {
        out.extend(junk_deletes(items, &grouped, rules));
    }
    out
}

/// The near-duplicate groups, each with the rule that joined them.
pub fn groups(kind: Kind, items: &[Item]) -> Vec<(&'static str, Vec<Item>)> {
    match kind {
        Kind::Author => author_components(items)
            .into_iter()
            .filter(|group| distinct_names(group) >= 2)
            .map(|group| ("author", group))
            .collect(),
        Kind::Publisher => keyed_groups(items, publisher_key)
            .into_iter()
            .map(|group| ("publisher_family", group))
            .collect(),
        Kind::Tag => keyed_groups(items, tag_key)
            .into_iter()
            .map(|group| ("tag_norm", group))
            .collect(),
    }
}

/// The heading to keep out of a group: the one with the most works, then the
/// cleaner spelling of it.
pub fn pick_canonical(kind: Kind, group: &[Item]) -> Item {
    // The first of the highest-scoring, so a tie goes to the heading the
    // catalogue listed first.
    group
        .iter()
        .cloned()
        .reduce(|best, item| match score(kind, &item) > score(kind, &best) {
            true => item,
            false => best,
        })
        .expect("a group is never empty")
}

fn score(kind: Kind, item: &Item) -> (i64, usize) {
    let name = &item.name;
    let mut score = item.count * 10;

    if is_shouting(name) && name.contains(' ') {
        score -= 6;
    }
    match kind {
        Kind::Author => {
            if name.contains(',') || name.contains('|') {
                score -= 8;
            }
            if name
                .trim_end_matches(textkit::is_space)
                .ends_with([';', '|', ','])
            {
                score -= 10;
            }
            if INITIALS_TOGETHER.is_match(name) {
                score -= 1;
            }
            if INITIALS_SPACED.is_match(name) {
                score += 3;
            }
        }
        Kind::Publisher => {
            if LEGAL_SUFFIX.is_match(name).unwrap_or(false) {
                score -= 4;
            }
            if name.contains(',') {
                score -= 2;
            }
        }
        Kind::Tag => {
            if name.starts_with(['[', '*', '&']) {
                score -= 10;
            }
            if name.ends_with('.') {
                score -= 4;
            }
        }
    }
    (score, name.chars().count())
}

/// Whether a heading is in capitals throughout, by Python's reading: at least
/// one cased letter, and no lowercase one.
fn is_shouting(name: &str) -> bool {
    name.chars().any(char::is_uppercase) && !name.chars().any(char::is_lowercase)
}

/// One proposal per alias in a group.
pub fn proposals_from_group(kind: Kind, group: &[Item], method: &str) -> Vec<Proposal> {
    let keep = pick_canonical(kind, group);
    let canon = keep.name.clone();

    group
        .iter()
        .filter(|item| item.name != canon)
        .map(|item| {
            let same = normalise(&item.name) == normalise(&canon);
            let (row_method, confidence) = match method {
                // An author group is joined two ways at once, and which one
                // joined this row decides how far to trust it: a heading that
                // normalises to the canonical one is the same string written
                // differently, where an inverted name is an inference.
                "author" => match same {
                    true => ("strict_norm", 0.95),
                    false => ("last_first", 0.9),
                },
                "publisher_family" => ("publisher_family", 0.8),
                other => (other, 0.9),
            };
            Proposal {
                kind: kind.name().to_string(),
                action: "merge".into(),
                keep: Some(canon.clone()),
                drop: item.name.clone(),
                keep_id: Some(keep.id),
                drop_id: item.id,
                keep_count: keep.count,
                drop_count: item.count,
                method: row_method.to_string(),
                confidence,
                evidence: format!(
                    "{} books as '{}' → '{}' ({})",
                    item.count, item.name, canon, keep.count
                ),
            }
        })
        .collect()
}

/// Why a subject heading should be deleted, or `None` to keep it.
///
/// High precision only: identifiers, catalogue leftovers, links, blurbs used
/// as headings, bare years and page counts. A real subject — a century, a
/// discipline, a war — must never match.
pub fn junk_reason(name: &str, rules: &Rules) -> Option<&'static str> {
    let raw = name.trim_matches(textkit::is_space);
    if raw.is_empty() {
        return Some("empty");
    }
    if PUNCTUATION_ONLY.is_match(raw) {
        return Some("punctuation-only");
    }
    let unbracketed = raw.trim_matches(['[', ']', '*', '&']);
    if FORMAT_OR_WATERMARK.is_match(raw) || FORMAT_OR_WATERMARK.is_match(unbracketed) {
        return Some("format-or-watermark");
    }
    if raw.starts_with('#') || CATALOGUE_FIELD.is_match(raw).unwrap_or(false) {
        return Some("catalog-field");
    }
    if URL.is_match(raw) {
        return Some("url");
    }
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    if ISBN_IN_TAG.is_match(raw).unwrap_or(false)
        && (raw.to_uppercase().starts_with("ISBN")
            || raw.chars().take(3).collect::<String>() == "978"
            || raw.chars().take(3).collect::<String>() == "979"
            || digits.len() >= 10)
    {
        return Some("isbn");
    }
    if YEAR_ONLY.is_match(raw) || ISO_DATE.is_match(raw) {
        return Some("date");
    }
    if BARE_NUMBER.is_match(raw) {
        return Some("numeric-id");
    }
    if TIMESTAMP.is_match(raw) {
        return Some("timestamp");
    }
    if is_device_tag(raw, rules) {
        return Some("device-tag");
    }
    if raw.chars().count() > 80 {
        return Some("blurb");
    }
    if raw.matches(';').count() >= 3 {
        return Some("exploded-list");
    }
    None
}

fn is_device_tag(raw: &str, rules: &Rules) -> bool {
    let Some(rest) = raw.strip_prefix('[') else {
        return false;
    };
    let rest = rest.to_lowercase();
    DEVICE_PREFIXES
        .iter()
        .map(|prefix| prefix.to_string())
        .chain(rules.device_prefixes.iter().map(|p| p.to_lowercase()))
        .any(|prefix| rest.starts_with(&prefix))
}

fn junk_deletes(items: &[Item], grouped: &[i64], rules: &Rules) -> Vec<Proposal> {
    items
        .iter()
        .filter(|item| !grouped.contains(&item.id))
        .filter_map(|item| {
            let why = junk_reason(&item.name, rules)?;
            Some(Proposal {
                kind: "tag".into(),
                action: "delete".into(),
                keep: None,
                drop: item.name.clone(),
                keep_id: None,
                drop_id: item.id,
                keep_count: 0,
                drop_count: item.count,
                method: format!("junk_tag:{why}"),
                confidence: 0.92,
                evidence: format!("{why}: '{}' on {} books", item.name, item.count),
            })
        })
        .collect()
}

/// A heading reduced to the letters and digits in it, accents removed.
pub fn normalise(name: &str) -> String {
    let stripped: String = name
        .nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect();
    let lowered = stripped.to_lowercase().replace('|', " ");
    NOT_ALPHANUMERIC
        .replace_all(&lowered, " ")
        .trim_matches(textkit::is_space)
        .to_string()
}

/// The words of a person's name, with roles and suffixes taken out.
pub fn author_tokens(name: &str) -> Vec<String> {
    let normalised = normalise(name);
    let without_roles = AUTHOR_ROLE.replace_all(&normalised, " ");
    without_roles
        .split_whitespace()
        .filter(|token| NAME_SUFFIXES.binary_search(token).is_err())
        .map(str::to_string)
        .collect()
}

/// A publisher reduced to what actually names it.
pub fn publisher_key(name: &str) -> String {
    let normalised = normalise(name);
    let without_noise = PUBLISHER_NOISE.replace_all(&normalised, " ");
    SPACE_RUN
        .replace_all(&without_noise, " ")
        .trim_matches(textkit::is_space)
        .to_string()
}

/// A subject heading reduced to what it is about.
pub fn tag_key(name: &str) -> String {
    let trimmed = name.trim_matches(textkit::is_space);
    let unbracketed = trimmed.trim_matches(['[', ']', '*', '&']);
    let trimmed = unbracketed.trim_matches(textkit::is_space);
    normalise(trimmed.trim_end_matches('.'))
}

/// Groups of two or more distinct headings sharing a key, in the order the
/// keys were first seen.
fn keyed_groups(items: &[Item], key: fn(&str) -> String) -> Vec<Vec<Item>> {
    let mut order: Vec<String> = Vec::new();
    let mut buckets: HashMap<String, Vec<Item>> = HashMap::new();

    for item in items {
        let key = key(&item.name);
        if key.is_empty() {
            continue;
        }
        buckets
            .entry(key.clone())
            .or_insert_with(|| {
                order.push(key);
                Vec::new()
            })
            .push(item.clone());
    }
    order
        .into_iter()
        .filter_map(|key| buckets.remove(&key))
        .filter(|group| distinct_names(group) >= 2)
        .collect()
}

/// Author groups, joining spelling matches and inverted names together.
///
/// Joined rather than kept apart, so the heading that is kept is chosen from
/// the whole set. Otherwise two surname-first spellings merge onto each other
/// and neither ever reaches the given-name-first one that most of the works
/// are actually under.
fn author_components(items: &[Item]) -> Vec<Vec<Item>> {
    let mut parent: HashMap<i64, i64> = items.iter().map(|item| (item.id, item.id)).collect();

    fn find(parent: &mut HashMap<i64, i64>, of: i64) -> i64 {
        let mut at = of;
        while parent[&at] != at {
            let grandparent = parent[&parent[&at]];
            parent.insert(at, grandparent);
            at = grandparent;
        }
        at
    }
    fn union(parent: &mut HashMap<i64, i64>, a: i64, b: i64) {
        let (a, b) = (find(parent, a), find(parent, b));
        if a != b {
            parent.insert(b, a);
        }
    }

    // Same spelling once the accents and punctuation are gone.
    for group in keyed_buckets(items, normalise) {
        for item in &group[1..] {
            union(&mut parent, group[0].id, item.id);
        }
    }

    // Same words in a different order, for the headings that read as people.
    let inverted = keyed_buckets(items, |name| {
        if !crate::reasons::looks_like_person(&name.replace('|', ",")) {
            return String::new();
        }
        let mut tokens = author_tokens(name);
        tokens.sort();
        match (2..=5).contains(&tokens.len()) {
            true => tokens.join(" "),
            false => String::new(),
        }
    });
    for group in inverted {
        for item in &group[1..] {
            union(&mut parent, group[0].id, item.id);
        }
    }

    let mut order: Vec<i64> = Vec::new();
    let mut components: HashMap<i64, Vec<Item>> = HashMap::new();
    for item in items {
        let root = find(&mut parent, item.id);
        components
            .entry(root)
            .or_insert_with(|| {
                order.push(root);
                Vec::new()
            })
            .push(item.clone());
    }
    order
        .into_iter()
        .filter_map(|root| components.remove(&root))
        .collect()
}

/// Every bucket of a key, including the ones with a single member, in
/// first-seen order.
fn keyed_buckets(items: &[Item], key: impl Fn(&str) -> String) -> Vec<Vec<Item>> {
    let mut order: Vec<String> = Vec::new();
    let mut buckets: HashMap<String, Vec<Item>> = HashMap::new();
    for item in items {
        let key = key(&item.name);
        if key.is_empty() {
            continue;
        }
        buckets
            .entry(key.clone())
            .or_insert_with(|| {
                order.push(key);
                Vec::new()
            })
            .push(item.clone());
    }
    order
        .into_iter()
        .filter_map(|key| buckets.remove(&key))
        .collect()
}

fn distinct_names(group: &[Item]) -> usize {
    let mut names: Vec<&str> = group.iter().map(|item| item.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, name: &str, count: i64) -> Item {
        Item {
            id,
            name: name.into(),
            count,
        }
    }

    #[test]
    fn the_suffix_list_is_sorted_so_it_can_be_searched() {
        assert!(NAME_SUFFIXES.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn normalising_removes_accents_and_punctuation() {
        assert_eq!(normalise("Dostoevsky| Fyodor"), "dostoevsky fyodor");
        assert_eq!(normalise("Étienne Gilson"), "etienne gilson");
        assert_eq!(normalise(""), "");
    }

    #[test]
    fn a_publisher_is_reduced_to_what_names_it() {
        assert_eq!(publisher_key("Oxford University Press, Inc."), "oxford");
        assert_eq!(publisher_key("Oxford University Press"), "oxford");
        assert_eq!(publisher_key("Routledge"), "routledge");
    }

    #[test]
    fn a_subject_is_reduced_to_what_it_is_about() {
        assert_eq!(tag_key("[Philosophy]"), "philosophy");
        assert_eq!(tag_key("Philosophy."), "philosophy");
        assert_eq!(tag_key("  *Philosophy  "), "philosophy");
    }

    #[test]
    fn a_role_is_not_part_of_a_name() {
        assert_eq!(author_tokens("Smith, John (editor)"), ["smith", "john"]);
        assert_eq!(author_tokens("Smith, John Jr."), ["smith", "john"]);
    }

    #[test]
    fn the_heading_with_the_most_works_is_the_one_kept() {
        let group = [
            item(1, "Dostoevsky, Fyodor", 3),
            item(2, "Fyodor Dostoevsky", 11),
        ];
        assert_eq!(
            pick_canonical(Kind::Author, &group).name,
            "Fyodor Dostoevsky"
        );
    }

    #[test]
    fn an_inverted_name_and_a_stray_separator_join_the_same_group() {
        let items = [
            item(1, "Dostoevsky, Fyodor", 3),
            item(2, "Dostoevsky| Fyodor", 1),
            item(3, "Fyodor Dostoevsky", 11),
        ];
        let proposals = cluster(Kind::Author, &items, &Rules::default());
        assert_eq!(proposals.len(), 2);
        assert!(
            proposals
                .iter()
                .all(|p| p.keep.as_deref() == Some("Fyodor Dostoevsky")),
            "{proposals:#?}"
        );
        // Neither surname-first spelling normalises to the given-name-first
        // heading, so both are inferences rather than respellings.
        assert!(
            proposals.iter().all(|p| p.method == "last_first"),
            "{proposals:#?}"
        );
    }

    #[test]
    fn a_respelling_is_trusted_further_than_an_inversion() {
        let items = [
            item(1, "Fyodor Dostoevsky", 11),
            item(2, "Fyodor Dostoëvsky", 2),
            item(3, "Dostoevsky, Fyodor", 3),
        ];
        let proposals = cluster(Kind::Author, &items, &Rules::default());
        let respelling = proposals
            .iter()
            .find(|p| p.drop == "Fyodor Dostoëvsky")
            .expect("a proposal");
        let inversion = proposals
            .iter()
            .find(|p| p.drop == "Dostoevsky, Fyodor")
            .expect("a proposal");
        assert_eq!(respelling.method, "strict_norm");
        assert_eq!(respelling.confidence, 0.95);
        assert_eq!(inversion.method, "last_first");
        assert_eq!(inversion.confidence, 0.9);
    }

    #[test]
    fn a_publisher_family_merges_onto_its_commonest_spelling() {
        let items = [
            item(1, "Oxford University Press", 40),
            item(2, "Oxford University Press, Inc.", 2),
            item(3, "Routledge", 9),
        ];
        let proposals = cluster(Kind::Publisher, &items, &Rules::default());
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].drop, "Oxford University Press, Inc.");
        assert_eq!(
            proposals[0].keep.as_deref(),
            Some("Oxford University Press")
        );
        assert_eq!(proposals[0].confidence, 0.8);
    }

    #[test]
    fn a_heading_on_its_own_is_left_alone() {
        let items = [item(1, "Philosophy", 30)];
        assert!(cluster(Kind::Tag, &items, &Rules::default()).is_empty());
    }

    #[test]
    fn a_subject_that_is_not_a_subject_is_proposed_for_deletion() {
        let rules = Rules::default();
        for (name, why) in [
            ("", "empty"),
            ("---", "punctuation-only"),
            ("z-lib.org", "format-or-watermark"),
            ("[kindle]", "format-or-watermark"),
            ("#genre", "format-or-watermark"),
            ("ISBN-13", "catalog-field"),
            ("https://example.org/x", "url"),
            ("9780198237907", "isbn"),
            ("1998", "date"),
            ("2024-01-15", "date"),
            ("451", "numeric-id"),
            ("1:23:45", "timestamp"),
            ("[kobo] favourites", "device-tag"),
            ("a;b;c;d", "exploded-list"),
        ] {
            assert_eq!(junk_reason(name, &rules), Some(why), "{name:?}");
        }
    }

    #[test]
    fn a_real_subject_is_never_junk() {
        let rules = Rules::default();
        for name in [
            "18th Century",
            "Philosophy",
            "World War, 1939-1945",
            "Church history",
            "Russia — History — Revolution, 1917-1921",
        ] {
            assert_eq!(junk_reason(name, &rules), None, "{name:?}");
        }
    }

    #[test]
    fn a_device_named_after_its_owner_comes_from_the_rules() {
        let rules = Rules {
            device_prefixes: vec!["alexskobo".into()],
        };
        assert_eq!(junk_reason("[alexskobo] to read", &Rules::default()), None);
        assert_eq!(
            junk_reason("[alexskobo] to read", &rules),
            Some("device-tag")
        );
    }

    #[test]
    fn a_heading_already_in_a_group_is_not_also_deleted() {
        let items = [item(1, "Philosophy", 30), item(2, "[Philosophy]", 1)];
        let proposals = cluster(Kind::Tag, &items, &Rules::default());
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].action, "merge");
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(cluster(Kind::Author, &[], &Rules::default()).is_empty());
    }
}
