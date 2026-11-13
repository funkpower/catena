//! Books by their title, for when there is no identifier to ask about.
//!
//! A book that arrives as a file often carries no ISBN at all, only a title
//! and perhaps a name. A register can still be asked which books it has under
//! those, and what it answers is a list of candidates: each an edition the
//! register holds, with whatever identifier, publisher and cover it gives.
//!
//! Which candidate is the book in hand is not decided here. The candidates
//! are what the register said, in the order it said them; deciding whether
//! one is the same work is a comparison of titles, and that belongs to
//! whoever holds the book.

use serde::Deserialize;

use crate::source::{self, GoogleIdentifier, GoogleReply, SUBJECT_LIMIT, Source};
use crate::{Error, Record};

/// The most candidates one search asks for. Google Books answers with no more
/// than forty at a time, and a title that needs more than forty candidates to
/// find the right one needs a better title.
pub const MAX_LIMIT: usize = 40;

/// What Open Library's index is asked to say about each work, and about the
/// one edition of it that best matches the search.
///
/// Not the work's own `cover_i` or `cover_edition_key`: the cover the index
/// chose for a work is often another edition's. The edition's `cover_i` is
/// the first cover in that edition's record, which is what a lookup by ISBN
/// gives as `cover_id`.
const OPEN_LIBRARY_FIELDS: &str = "key,title,subtitle,author_name,first_publish_year,publisher,\
isbn,language,subject,number_of_pages_median,editions,editions.key,editions.isbn,\
editions.publisher,editions.language,editions.publish_date,editions.cover_i";

/// Ask which books a register has under a title, and by an author if one is
/// named.
///
/// Open Library's index first, and Google Books only when that answered with
/// nothing. A register being down is a failure only when every register
/// asked is: Open Library answering that it has nothing is an answer, and it
/// stands when Google Books cannot be reached. Asks nothing when `limit` is
/// zero; a `limit` over [`MAX_LIMIT`] is taken as that.
pub fn search(
    agent: &ureq::Agent,
    title: &str,
    author: Option<&str>,
    limit: usize,
    attempts: u32,
) -> Result<Vec<Record>, Error> {
    search_with(title, author, limit, |url| {
        crate::ask::get(agent, url, attempts)
    })
}

/// [`search`], with how to fetch an address passed in.
fn search_with(
    title: &str,
    author: Option<&str>,
    limit: usize,
    mut get: impl FnMut(&str) -> Result<Option<String>, Error>,
) -> Result<Vec<Record>, Error> {
    let title = title.trim();
    if title.is_empty() {
        return Err(Error::Invalid("no title to search for".into()));
    }
    let author = author.map(str::trim).filter(|author| !author.is_empty());
    let limit = limit.min(MAX_LIMIT);
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut fetch = |url: String| match get(&url)? {
        Some(body) => Ok(body),
        // A search that matches nothing answers with an empty list. A 404 is
        // the address being wrong, not the book being absent.
        None => Err(Error::Unreachable(format!("{url}: answered 404"))),
    };

    let open_library =
        fetch(open_library_url(title, author, limit)).and_then(|body| open_library(&body, limit));
    if open_library.as_ref().is_ok_and(|found| !found.is_empty()) {
        return open_library;
    }

    let google = match google_url(title, author, limit) {
        Some(url) => fetch(url).and_then(|body| google_books(&body, limit)),
        // Nothing in the title Google could be asked by.
        None => Ok(Vec::new()),
    };
    match (open_library, google) {
        (_, Ok(found)) if !found.is_empty() => Ok(found),
        (Ok(none), _) => Ok(none),
        (Err(why), _) => Err(why),
    }
}

/// Where to ask Open Library.
fn open_library_url(title: &str, author: Option<&str>, limit: usize) -> String {
    let author = author
        .map(|author| format!("&author={}", source::encode(author)))
        .unwrap_or_default();
    format!(
        "https://openlibrary.org/search.json?title={}{author}&fields={OPEN_LIBRARY_FIELDS}&limit={limit}",
        source::encode(title),
    )
}

/// Where to ask Google Books, or `None` when there is nothing to ask by.
///
/// Every word is its own `intitle:` or `inauthor:` term, so no character of
/// the title can open a phrase, negate a term or name another field: a word is
/// the letters, digits and apostrophes in it, and anything else separates
/// words.
fn google_url(title: &str, author: Option<&str>, limit: usize) -> Option<String> {
    let terms: Vec<String> = words(title)
        .map(|word| format!("intitle:{word}"))
        .chain(
            author
                .into_iter()
                .flat_map(words)
                .map(|word| format!("inauthor:{word}")),
        )
        .collect();
    if !terms.iter().any(|term| term.starts_with("intitle:")) {
        return None;
    }
    Some(format!(
        "https://www.googleapis.com/books/v1/volumes?q={}&maxResults={limit}",
        source::encode(&terms.join(" "))
    ))
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .map(|word| word.trim_matches('\''))
        .filter(|word| !word.is_empty())
}

/// The first ISBN in a list whose check digit agrees, in its thirteen-digit
/// form. One written in thirteen digits is taken over one written in ten.
fn first_isbn<'a>(isbns: impl Iterator<Item = &'a str> + Clone) -> Option<String> {
    isbns
        .clone()
        .filter(|isbn| idnorm::isbn::digits(isbn).len() == 13)
        .find_map(idnorm::isbn::to_13)
        .or_else(|| isbns.into_iter().find_map(idnorm::isbn::to_13))
}

#[derive(Deserialize)]
struct OpenLibraryReply {
    #[serde(default)]
    docs: Vec<Work>,
}

#[derive(Deserialize)]
struct Work {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    author_name: Vec<String>,
    #[serde(default)]
    first_publish_year: Option<i64>,
    #[serde(default)]
    publisher: Vec<String>,
    #[serde(default)]
    isbn: Vec<String>,
    #[serde(default)]
    language: Vec<String>,
    #[serde(default)]
    subject: Vec<String>,
    #[serde(default)]
    number_of_pages_median: Option<u32>,
    #[serde(default)]
    editions: Option<Editions>,
}

#[derive(Deserialize)]
struct Editions {
    #[serde(default)]
    docs: Vec<Edition>,
}

#[derive(Deserialize)]
struct Edition {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    isbn: Vec<String>,
    #[serde(default)]
    publisher: Vec<String>,
    #[serde(default)]
    language: Vec<String>,
    #[serde(default)]
    publish_date: Vec<String>,
    #[serde(default)]
    cover_i: Option<i64>,
}

/// The last segment of an Open Library key: `OL3585423M` out of
/// `/books/OL3585423M`. The letter at the end says what it names, `M` an
/// edition and `W` a work.
fn olid(key: &str) -> Option<String> {
    key.rsplit('/')
        .next()
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// Read Open Library's answer into one candidate per work, in its order.
///
/// What belongs to an edition — its ISBN, publisher, language, year and
/// cover — is taken from the edition the index chose as matching best; what
/// belongs to the work — its title, authors, subjects and length — from the
/// work. A work the index names no edition of gives its first ISBN that reads,
/// which may be any edition's, and no cover id.
fn open_library(body: &str, limit: usize) -> Result<Vec<Record>, Error> {
    let reply: OpenLibraryReply =
        serde_json::from_str(body).map_err(|e| Error::Shape(e.to_string()))?;
    Ok(reply
        .docs
        .into_iter()
        .filter_map(candidate)
        .take(limit)
        .collect())
}

fn candidate(work: Work) -> Option<Record> {
    let edition = work.editions.and_then(|e| e.docs.into_iter().next());

    // The cover is kept only when the record names the edition it is the
    // cover of, by that edition's ISBN or its key.
    let (id, kind, edition_named) = match &edition {
        Some(edition) => match first_isbn(edition.isbn.iter().map(String::as_str)) {
            Some(isbn) => (isbn, "isbn", true),
            None => match edition.key.as_deref().and_then(olid) {
                Some(key) => (key, "olid", true),
                None => (work.key.as_deref().and_then(olid)?, "olid", false),
            },
        },
        None => match first_isbn(work.isbn.iter().map(String::as_str)) {
            Some(isbn) => (isbn, "isbn", false),
            // A work with no key cannot be named, so it is not a candidate.
            None => (work.key.as_deref().and_then(olid)?, "olid", false),
        },
    };

    let mut record = source::blank(&id, kind, Source::OpenLibrarySearch);
    record.title = match (work.title, work.subtitle) {
        (Some(title), Some(subtitle)) => Some(format!("{title}: {subtitle}")),
        (title, _) => title,
    };
    record.authors = work.author_name;
    record.subjects = work.subject;
    record.subjects.truncate(SUBJECT_LIMIT);
    record.pages = work.number_of_pages_median;

    let (publisher, language, year) = match edition {
        Some(edition) => {
            if edition_named {
                record.cover_id = edition
                    .cover_i
                    .and_then(|id| u64::try_from(id).ok())
                    .filter(|id| *id > 0);
            }
            (
                edition.publisher.into_iter().next(),
                edition.language.into_iter().next(),
                edition
                    .publish_date
                    .iter()
                    .find_map(|date| source::year_in(date)),
            )
        }
        None => (None, None, None),
    };
    record.container = publisher.or_else(|| work.publisher.into_iter().next());
    record.language = language.or_else(|| work.language.into_iter().next());
    record.year = year.or(work.first_publish_year);
    Some(record)
}

/// Read Google Books' answer into one candidate per volume, in its order.
fn google_books(body: &str, limit: usize) -> Result<Vec<Record>, Error> {
    let reply: GoogleReply = serde_json::from_str(body).map_err(|e| Error::Shape(e.to_string()))?;
    Ok(reply
        .items
        .into_iter()
        .filter_map(|item| {
            // Only what Google types as an ISBN: its `OTHER` identifiers are
            // library shelfmarks, some of which are ten digits long.
            let isbns = item
                .volume_info
                .identifiers
                .iter()
                .filter(|GoogleIdentifier { kind, .. }| kind == "ISBN_13" || kind == "ISBN_10")
                .map(|identifier| identifier.identifier.as_str());
            let (id, kind) = match first_isbn(isbns) {
                Some(isbn) => (isbn, "isbn"),
                None => (item.id.filter(|id| !id.is_empty())?, "googlebooks"),
            };
            Some(source::google_volume(&id, kind, item.volume_info))
        })
        .take(limit)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open Library's answer to `title=the first world war&author=howard`,
    /// recorded 2026-11-13 with the fields above and `limit=2`, the ISBN and
    /// subject lists shortened.
    const HOWARD: &str = r#"{"numFound": 9, "start": 0, "numFoundExact": true, "num_found": 9,
        "documentation_url": "https://openlibrary.org/dev/docs/api/search", "q": "", "offset": null,
        "docs": [
          {"author_name": ["Michael Eliot Howard"], "first_publish_year": 2002,
           "isbn": ["9780192853622", "0192853627"], "key": "/works/OL1382726W",
           "language": ["eng"], "number_of_pages_median": 154,
           "publisher": ["Oxford University Press"], "title": "The First World War",
           "subject": ["World War, 1914-1918"],
           "editions": {"numFound": 1, "start": 0, "numFoundExact": true, "docs": [
             {"key": "/books/OL3585423M", "cover_i": 1123124, "language": ["eng"],
              "publisher": ["Oxford University Press"], "publish_date": ["2002"],
              "isbn": ["0192853627", "9780192853622"]}]}},
          {"author_name": ["Michael Howard"], "first_publish_year": 2007,
           "isbn": ["0199205590", "9780199205592"], "key": "/works/OL4282399W",
           "language": ["eng"], "publisher": ["Oxford University Press"],
           "subtitle": "A Very Short Introduction", "title": "The First World War",
           "editions": {"numFound": 1, "start": 0, "numFoundExact": true, "docs": [
             {"key": "/books/OL7403004M", "cover_i": 14606475, "language": ["eng"],
              "publisher": ["Oxford University Press"], "publish_date": ["20 May 2007"],
              "isbn": ["9780199205592", "0199205590"]}]},
           "subject": []}]}"#;

    /// Open Library's answer to `title=war and peace&author=tolstoy`, recorded
    /// 2026-11-13, first work only. The edition the index matched best is a
    /// 1917 printing that has no ISBN, though other editions of the work do.
    const TOLSTOY: &str = r#"{"numFound": 146, "docs": [
          {"author_name": ["Leo Tolstoy"], "first_publish_year": 1867,
           "isbn": ["9781524881993", "1645178889"], "key": "/works/OL267171W",
           "title": "War and Peace",
           "editions": {"numFound": 487, "docs": [
             {"key": "/books/OL37863121M", "cover_i": 12736340, "language": ["eng"],
              "publisher": ["Thomas Y. Crowell"], "publish_date": ["1917"]}]}}]}"#;

    /// Open Library's answer when nothing has the title.
    const NOTHING: &str = r#"{"numFound": 0, "start": 0, "numFoundExact": true, "num_found": 0,
        "documentation_url": "https://openlibrary.org/dev/docs/api/search", "q": "",
        "offset": null, "docs": []}"#;

    /// Google Books' answer for a volume, in the shape its API documents.
    const GOOGLE: &str = r#"{"kind": "books#volumes", "totalItems": 2, "items": [
          {"kind": "books#volume", "id": "zyTCAlFPjgYC", "volumeInfo": {
             "title": "The Google Story", "authors": ["David A. Vise", "Mark Malseed"],
             "publisher": "Random House Digital, Inc.", "publishedDate": "2005-11-15",
             "industryIdentifiers": [
               {"type": "ISBN_10", "identifier": "055380457X"},
               {"type": "ISBN_13", "identifier": "9780553804577"}],
             "pageCount": 207, "categories": ["Browsers (Computer programs)"],
             "language": "en"}},
          {"kind": "books#volume", "id": "a1b2C3d4", "volumeInfo": {
             "title": "The Google Story", "subtitle": "A Dissertation",
             "publishedDate": "2009",
             "industryIdentifiers": [
               {"type": "OTHER", "identifier": "UOM:0306406152"}]}}]}"#;

    /// Fetch from a table of recorded bodies, keeping every address asked.
    fn recorded<'a>(
        asked: &'a mut Vec<String>,
        bodies: &'a [(&'a str, Result<&'a str, Error>)],
    ) -> impl FnMut(&str) -> Result<Option<String>, Error> + 'a {
        move |url| {
            asked.push(url.to_string());
            let (_, body) = bodies
                .iter()
                .find(|(host, _)| url.contains(host))
                .unwrap_or_else(|| panic!("asked an address nothing was recorded for: {url}"));
            body.clone().map(|body| Some(body.to_string()))
        }
    }

    fn down() -> Error {
        Error::Unreachable("connection refused".into())
    }

    #[test]
    fn each_work_open_library_has_is_a_candidate_named_by_its_isbn() {
        let mut asked = Vec::new();
        let bodies = [("openlibrary.org", Ok(HOWARD))];
        let found = search_with(
            "The First World War",
            Some("Howard"),
            5,
            recorded(&mut asked, &bodies),
        )
        .expect("candidates");
        assert_eq!(
            asked.len(),
            1,
            "Google is not asked when Open Library answers"
        );

        assert_eq!(found.len(), 2);
        let first = &found[0];
        assert!(first.found);
        assert_eq!(first.v, corpus_core::V);
        assert_eq!(first.source, "openlibrary");
        // Written ten-digit first by the edition, given in thirteen.
        assert_eq!(
            (first.id.as_str(), first.kind.as_str()),
            ("9780192853622", "isbn")
        );
        assert_eq!(first.title.as_deref(), Some("The First World War"));
        assert_eq!(first.authors, ["Michael Eliot Howard"]);
        assert_eq!(first.year, Some(2002));
        assert_eq!(first.container.as_deref(), Some("Oxford University Press"));
        assert_eq!(first.language.as_deref(), Some("eng"));
        assert_eq!(first.subjects, ["World War, 1914-1918"]);
        assert_eq!(first.pages, Some(154));
        assert_eq!(first.cover_id, Some(1123124));

        let second = &found[1];
        assert_eq!(second.id, "9780199205592");
        assert_eq!(
            second.title.as_deref(),
            Some("The First World War: A Very Short Introduction")
        );
        // The edition's year, not the work's first.
        assert_eq!(second.year, Some(2007));
        // The first cover in OL7403004M's own record; the work's cover is
        // 1137779, which is not.
        assert_eq!(second.cover_id, Some(14606475));
    }

    #[test]
    fn the_candidates_stay_in_the_order_the_register_gave_them() {
        let mut asked = Vec::new();
        let bodies = [("openlibrary.org", Ok(HOWARD))];
        let found =
            search_with("first world war", None, 5, recorded(&mut asked, &bodies)).expect("ok");
        let ids: Vec<&str> = found.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["9780192853622", "9780199205592"]);
        let found =
            search_with("first world war", None, 1, recorded(&mut asked, &bodies)).expect("ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "9780192853622");
    }

    #[test]
    fn an_edition_with_no_isbn_is_named_by_its_key_and_keeps_its_cover() {
        let mut asked = Vec::new();
        let bodies = [("openlibrary.org", Ok(TOLSTOY))];
        let found = search_with(
            "War and Peace",
            Some("Tolstoy"),
            5,
            recorded(&mut asked, &bodies),
        )
        .expect("candidates");
        let only = &found[0];
        // Not 9781524881993: that is another edition's, and the publisher,
        // year and cover here are this one's.
        assert_eq!(
            (only.id.as_str(), only.kind.as_str()),
            ("OL37863121M", "olid")
        );
        assert_eq!(only.container.as_deref(), Some("Thomas Y. Crowell"));
        assert_eq!(only.year, Some(1917));
        assert_eq!(only.cover_id, Some(12736340));
    }

    #[test]
    fn a_work_with_no_edition_named_gives_its_isbn_but_no_cover() {
        let body = r#"{"docs": [{"key": "/works/OL1W", "title": "A Book",
            "isbn": ["0-306-40615-2"], "publisher": ["Plenum"], "first_publish_year": 1981}]}"#;
        let found = open_library(body, 5).expect("candidates");
        assert_eq!(
            (found[0].id.as_str(), found[0].kind.as_str()),
            ("9780306406157", "isbn")
        );
        assert_eq!(found[0].container.as_deref(), Some("Plenum"));
        assert_eq!(found[0].year, Some(1981));
        assert_eq!(found[0].cover_id, None);
    }

    #[test]
    fn a_work_with_no_isbn_and_no_edition_is_named_by_the_works_key() {
        let body = r#"{"docs": [{"key": "/works/OL267171W", "title": "War and Peace"}]}"#;
        let found = open_library(body, 5).expect("candidates");
        assert_eq!(
            (found[0].id.as_str(), found[0].kind.as_str()),
            ("OL267171W", "olid")
        );
        assert_eq!(found[0].cover_id, None);
    }

    #[test]
    fn isbns_whose_check_digit_is_wrong_are_passed_over() {
        // 9780306406158 and 0306406153 are 9780306406157 mistyped.
        let body = r#"{"docs": [
            {"key": "/works/OL1W", "title": "A", "editions": {"docs": [
              {"key": "/books/OL1M", "isbn": ["9780306406158", "0306406153"], "cover_i": 7}]}},
            {"key": "/works/OL2W", "title": "B", "editions": {"docs": [
              {"key": "/books/OL2M", "isbn": ["9780306406158", "0306406152"]}]}}]}"#;
        let found = open_library(body, 5).expect("candidates");
        assert_eq!(
            (found[0].id.as_str(), found[0].kind.as_str()),
            ("OL1M", "olid")
        );
        assert_eq!(found[0].cover_id, Some(7));
        // The ten-digit one reads, and is given in thirteen.
        assert_eq!(
            (found[1].id.as_str(), found[1].kind.as_str()),
            ("9780306406157", "isbn")
        );
    }

    #[test]
    fn thirteen_digits_are_taken_over_ten() {
        // Two different ISBNs on one edition: the thirteen-digit one wins
        // although the ten-digit one is listed first.
        let isbns = ["0306406152", "9780199205592"];
        assert_eq!(
            first_isbn(isbns.into_iter()).as_deref(),
            Some("9780199205592")
        );
        assert_eq!(first_isbn(["nonsense"].into_iter()), None);
    }

    #[test]
    fn nothing_from_open_library_is_asked_of_google_books() {
        let mut asked = Vec::new();
        let bodies = [
            ("openlibrary.org", Ok(NOTHING)),
            ("googleapis.com", Ok(GOOGLE)),
        ];
        let found = search_with(
            "The Google Story",
            Some("Vise"),
            5,
            recorded(&mut asked, &bodies),
        )
        .expect("candidates");
        assert_eq!(asked.len(), 2);
        assert!(asked[1].starts_with("https://www.googleapis.com/books/v1/volumes?q="));

        assert_eq!(found.len(), 2);
        let first = &found[0];
        assert_eq!(first.source, "googlebooks");
        assert_eq!(
            (first.id.as_str(), first.kind.as_str()),
            ("9780553804577", "isbn")
        );
        assert_eq!(first.authors, ["David A. Vise", "Mark Malseed"]);
        assert_eq!(first.year, Some(2005));
        assert_eq!(
            first.container.as_deref(),
            Some("Random House Digital, Inc.")
        );
        assert_eq!(first.pages, Some(207));
        // Google Books knows nothing of Open Library's covers.
        assert_eq!(first.cover_id, None);

        // A shelfmark is not an ISBN, though its digits would pass for one.
        let second = &found[1];
        assert_eq!(
            (second.id.as_str(), second.kind.as_str()),
            ("a1b2C3d4", "googlebooks")
        );
        assert_eq!(
            second.title.as_deref(),
            Some("The Google Story: A Dissertation")
        );
    }

    #[test]
    fn nothing_from_either_register_is_no_candidates() {
        let mut asked = Vec::new();
        let bodies = [
            ("openlibrary.org", Ok(NOTHING)),
            (
                "googleapis.com",
                Ok(r#"{"kind": "books#volumes", "totalItems": 0}"#),
            ),
        ];
        let found =
            search_with("Nothing", None, 5, recorded(&mut asked, &bodies)).expect("an answer");
        assert!(found.is_empty());
    }

    #[test]
    fn open_library_having_nothing_stands_when_google_books_is_down() {
        let mut asked = Vec::new();
        let bodies = [
            ("openlibrary.org", Ok(NOTHING)),
            ("googleapis.com", Err(down())),
        ];
        let found =
            search_with("Nothing", None, 5, recorded(&mut asked, &bodies)).expect("an answer");
        assert!(found.is_empty());
    }

    #[test]
    fn open_library_down_is_asked_of_google_books() {
        let mut asked = Vec::new();
        let bodies = [
            ("openlibrary.org", Err(down())),
            ("googleapis.com", Ok(GOOGLE)),
        ];
        let found =
            search_with("The Google Story", None, 5, recorded(&mut asked, &bodies)).expect("ok");
        assert_eq!(found[0].source, "googlebooks");
    }

    #[test]
    fn every_register_down_is_a_failure() {
        let mut asked = Vec::new();
        let bodies = [
            ("openlibrary.org", Err(down())),
            ("googleapis.com", Err(down())),
        ];
        let failed = search_with("A Book", None, 5, recorded(&mut asked, &bodies));
        assert!(matches!(failed, Err(Error::Unreachable(_))), "{failed:?}");
    }

    #[test]
    fn an_answer_that_cannot_be_read_says_so() {
        assert!(matches!(open_library("<html>", 5), Err(Error::Shape(_))));
        assert!(matches!(google_books("<html>", 5), Err(Error::Shape(_))));
    }

    #[test]
    fn the_author_is_left_out_when_there_is_none() {
        for author in [None, Some(""), Some("   ")] {
            let url = open_library_url(
                "Anathem",
                author.map(str::trim).filter(|a| !a.is_empty()),
                5,
            );
            assert!(!url.contains("author="), "{url}");
        }
        let mut asked = Vec::new();
        let bodies = [
            ("openlibrary.org", Ok(NOTHING)),
            ("googleapis.com", Ok(GOOGLE)),
        ];
        search_with("Anathem", Some("  "), 5, recorded(&mut asked, &bodies)).expect("ok");
        assert!(!asked[0].contains("author="), "{}", asked[0]);
        assert!(!asked[1].contains("inauthor"), "{}", asked[1]);
        assert!(asked[1].contains("q=intitle%3AAnathem&"), "{}", asked[1]);
    }

    #[test]
    fn the_author_narrows_the_search_when_there_is_one() {
        let url = open_library_url("Anathem", Some("Neal Stephenson"), 5);
        assert!(
            url.contains("?title=Anathem&author=Neal%20Stephenson&fields="),
            "{url}"
        );
        let url = google_url("Anathem", Some("Neal Stephenson"), 5).expect("a url");
        assert!(
            url.contains("q=intitle%3AAnathem%20inauthor%3ANeal%20inauthor%3AStephenson&"),
            "{url}"
        );
    }

    #[test]
    fn a_title_cannot_change_the_shape_of_the_address() {
        let hostile = r#"Fish & Chips: "A History" #2?=x/y+z"#;
        for url in [
            open_library_url(hostile, Some("O'Brien & Sons"), 5),
            google_url(hostile, Some("O'Brien & Sons"), 5).expect("a url"),
        ] {
            assert_eq!(url.matches('?').count(), 1, "{url}");
            assert_eq!(url.matches('#').count(), 0, "{url}");
            assert_eq!(url.matches('"').count(), 0, "{url}");
            assert_eq!(url.matches('+').count(), 0, "{url}");
            let query = url.split_once('?').expect("a query").1;
            let names: Vec<&str> = query
                .split('&')
                .map(|pair| pair.split_once('=').expect("name=value").0)
                .collect();
            assert!(
                names == ["title", "author", "fields", "limit"] || names == ["q", "maxResults"],
                "{url}"
            );
        }
        // Open Library is asked the title as written.
        let url = open_library_url(hostile, None, 5);
        assert!(
            url.contains("title=Fish%20%26%20Chips%3A%20%22A%20History%22%20%232%3F%3Dx%2Fy%2Bz&"),
            "{url}"
        );
        // Google is asked word by word, so nothing in a title is syntax.
        let url = google_url(hostile, Some("O'Brien & Sons"), 5).expect("a url");
        let q = "intitle:Fish intitle:Chips intitle:A intitle:History intitle:2 intitle:x \
intitle:y intitle:z inauthor:O'Brien inauthor:Sons";
        assert!(url.contains(&format!("q={}&", source::encode(q))), "{url}");
    }

    #[test]
    fn a_title_outside_ascii_is_sent_as_utf8() {
        let url = open_library_url("Преступление и наказание", Some("Dostoevský"), 5);
        assert!(
            url.contains("title=%D0%9F%D1%80%D0%B5%D1%81%D1%82%D1%83%D0%BF"),
            "{url}"
        );
        assert!(url.contains("author=Dostoevsk%C3%BD&"), "{url}");
        let url = google_url("Преступление и наказание", None, 5).expect("a url");
        // Letters in any script are words.
        assert!(url.contains("intitle%3A%D0%B8%20"), "{url}");
    }

    #[test]
    fn a_title_with_no_words_is_not_asked_of_google_books() {
        assert_eq!(google_url("?!", Some("Someone"), 5), None);
        let mut asked = Vec::new();
        let bodies = [("openlibrary.org", Ok(NOTHING))];
        let found = search_with("?!", Some("Someone"), 5, recorded(&mut asked, &bodies))
            .expect("an answer");
        assert!(found.is_empty());
        assert_eq!(asked.len(), 1);
    }

    #[test]
    fn no_title_is_refused_without_asking() {
        let mut asked = Vec::new();
        let failed = search_with("   ", Some("Someone"), 5, recorded(&mut asked, &[]));
        assert!(matches!(failed, Err(Error::Invalid(_))), "{failed:?}");
        assert!(asked.is_empty());
    }

    #[test]
    fn the_limit_is_asked_for_and_held_to() {
        let mut asked = Vec::new();
        let bodies = [
            ("openlibrary.org", Ok(NOTHING)),
            ("googleapis.com", Ok(GOOGLE)),
        ];
        let found =
            search_with("The Google Story", None, 1, recorded(&mut asked, &bodies)).expect("ok");
        assert_eq!(found.len(), 1);
        assert!(asked[0].ends_with("&limit=1"), "{}", asked[0]);
        assert!(asked[1].ends_with("&maxResults=1"), "{}", asked[1]);

        let mut asked = Vec::new();
        search_with("The Google Story", None, 500, recorded(&mut asked, &bodies)).expect("ok");
        assert!(asked[0].ends_with("&limit=40"), "{}", asked[0]);
        assert!(asked[1].ends_with("&maxResults=40"), "{}", asked[1]);

        let mut asked = Vec::new();
        let found =
            search_with("The Google Story", None, 0, recorded(&mut asked, &bodies)).expect("ok");
        assert!(found.is_empty());
        assert!(asked.is_empty());
    }

    #[test]
    fn the_index_is_not_asked_for_the_works_cover() {
        let url = open_library_url("A", None, 5);
        let fields = url
            .split('&')
            .find_map(|pair| pair.strip_prefix("fields="))
            .expect("fields");
        let fields: Vec<&str> = fields.split(',').collect();
        assert!(fields.contains(&"editions.cover_i"));
        assert!(!fields.contains(&"cover_i"));
        assert!(!fields.contains(&"cover_edition_key"));
    }

    #[test]
    fn a_404_from_a_search_is_a_failure_and_not_an_absence() {
        let failed = search_with("A Book", None, 5, |_| Ok(None));
        assert!(matches!(failed, Err(Error::Unreachable(_))), "{failed:?}");
    }
}
