//! The registers, and how to read what each one says.
//!
//! Each is the body that assigns the identifiers it answers about, so each is
//! the one place the answer is not a guess. The shapes they reply in have
//! nothing in common, which is most of what this module is.

use serde::Deserialize;

use crate::{Error, Record};

/// A register that can be asked about an identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Crossref, which assigns most scholarly DOIs.
    Crossref,
    /// The arXiv's own register of preprints.
    Arxiv,
    /// OpenLibrary, for books by ISBN.
    OpenLibrary,
    /// OpenLibrary's index, rather than its edition record. Asked when the
    /// edition record has the book but not who wrote it, which is often: an
    /// edition names its authors by key, and the index names them.
    OpenLibrarySearch,
    /// Google Books, for books by ISBN. Asked last, because it answers
    /// anonymous callers only until a daily quota runs out.
    GoogleBooks,
}

impl Source {
    /// What it is called.
    pub fn name(self) -> &'static str {
        match self {
            Source::Crossref => "crossref",
            Source::Arxiv => "arxiv",
            Source::OpenLibrary => "openlibrary",
            Source::OpenLibrarySearch => "openlibrary",
            Source::GoogleBooks => "googlebooks",
        }
    }

    /// Which register answers about a kind of identifier.
    pub fn for_kind(kind: &str) -> Option<Self> {
        Self::chain_for_kind(kind).first().copied()
    }

    /// Every register that answers about a kind, in the order to ask them.
    ///
    /// One register is enough for a DOI or a preprint: the body that assigns
    /// the identifier is the only one that has it. A book is different. An
    /// ISBN is assigned by a publisher and recorded by whoever gets round to
    /// it, so coverage is patchy in different places for different books,
    /// and an answer missing the authors is common enough to be worth asking
    /// a second register about.
    pub fn chain_for_kind(kind: &str) -> &'static [Self] {
        match kind {
            "doi" => &[Source::Crossref],
            "arxiv" => &[Source::Arxiv],
            // The index first. Asked for the fields by name it answers with
            // the title, the authors, the publisher, the language, a page
            // count and the subjects in one request, which is the whole of
            // what a catalogue wants; the edition record is then needed only
            // for a subtitle or a publisher the index did not give. Asking it
            // second cost three requests per book where one usually does, and
            // at three books in flight that is the difference between two
            // days and two hours.
            "isbn" => &[
                Source::OpenLibrarySearch,
                Source::OpenLibrary,
                Source::GoogleBooks,
            ],
            _ => &[],
        }
    }

    /// Where to ask about one identifier.
    pub fn url(self, id: &str) -> String {
        match self {
            Source::Crossref => format!("https://api.crossref.org/works/{}", encode(id)),
            Source::Arxiv => format!(
                "https://export.arxiv.org/api/query?id_list={}&max_results=1",
                encode(id)
            ),
            Source::OpenLibrary => {
                format!("https://openlibrary.org/isbn/{}.json", encode(id))
            }
            // `fields` or the index answers with a handful of defaults and
            // keeps the rest, which is how a language it holds went unread.
            Source::OpenLibrarySearch => format!(
                "https://openlibrary.org/search.json?q=isbn:{}\
&fields=title,subtitle,author_name,first_publish_year,publisher,language,subject,\
number_of_pages_median&limit=1",
                encode(id)
            ),
            Source::GoogleBooks => format!(
                "https://www.googleapis.com/books/v1/volumes?q=isbn:{}",
                encode(id)
            ),
        }
    }

    /// Read what it said into a record.
    pub fn read(self, id: &str, kind: &str, body: &str) -> Result<Record, Error> {
        match self {
            Source::Crossref => crossref(id, kind, body),
            Source::Arxiv => arxiv(id, kind, body),
            Source::OpenLibrary => open_library(id, kind, body),
            Source::OpenLibrarySearch => open_library_search(id, kind, body),
            Source::GoogleBooks => google_books(id, kind, body),
        }
    }
}

/// Percent-encode what a path or a query would otherwise read as structure.
///
/// A DOI suffix may hold a slash, a question mark or a hash, and every one of
/// those means something else in a URL.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            byte => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A record with the fields nothing found fills in.
fn blank(id: &str, kind: &str, source: Source) -> Record {
    let mut record = crate::missing(id, kind, source);
    record.found = true;
    record
}

#[derive(Deserialize)]
struct CrossrefReply {
    message: CrossrefWork,
}

#[derive(Deserialize)]
struct CrossrefWork {
    #[serde(default)]
    title: Vec<String>,
    #[serde(default)]
    author: Vec<CrossrefAuthor>,
    #[serde(default, rename = "container-title")]
    container_title: Vec<String>,
    #[serde(default)]
    issued: Option<CrossrefDate>,
    #[serde(default, rename = "abstract")]
    summary: Option<String>,
}

#[derive(Deserialize)]
struct CrossrefAuthor {
    #[serde(default)]
    given: Option<String>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct CrossrefDate {
    #[serde(default, rename = "date-parts")]
    date_parts: Vec<Vec<i64>>,
}

fn crossref(id: &str, kind: &str, body: &str) -> Result<Record, Error> {
    let reply: CrossrefReply =
        serde_json::from_str(body).map_err(|e| Error::Shape(e.to_string()))?;
    let work = reply.message;

    let mut record = blank(id, kind, Source::Crossref);
    record.title = work.title.into_iter().next();
    record.container = work.container_title.into_iter().next();
    record.year = work
        .issued
        .and_then(|issued| issued.date_parts.into_iter().next())
        .and_then(|parts| parts.into_iter().next());
    record.authors = work
        .author
        .into_iter()
        .filter_map(|author| match (author.given, author.family, author.name) {
            (Some(given), Some(family), _) => Some(format!("{given} {family}")),
            (None, Some(family), _) => Some(family),
            (_, _, Some(name)) => Some(name),
            _ => None,
        })
        .collect();
    // Crossref's abstracts arrive as JATS markup.
    record.summary = work.summary.map(|summary| strip_tags(&summary));
    Ok(record)
}

/// The arXiv answers in Atom, which is XML. Reading it properly would mean a
/// parser; what is wanted is four fields, and they are each one tag.
fn arxiv(id: &str, kind: &str, body: &str) -> Result<Record, Error> {
    // An empty feed means no such preprint, which the API signals by omitting
    // the entry rather than by a status code.
    let Some(entry) = between(body, "<entry>", "</entry>") else {
        return Ok(crate::missing(id, kind, Source::Arxiv));
    };

    let mut record = blank(id, kind, Source::Arxiv);
    record.title = between(entry, "<title>", "</title>").map(collapse);
    record.summary = between(entry, "<summary>", "</summary>").map(collapse);
    record.container = Some("arXiv".to_string());
    record.year = between(entry, "<published>", "</published>")
        .and_then(|published| published.get(..4))
        .and_then(|year| year.parse().ok());

    let mut authors = Vec::new();
    let mut rest = entry;
    while let Some(author) = between(rest, "<author>", "</author>") {
        if let Some(name) = between(author, "<name>", "</name>") {
            authors.push(collapse(name));
        }
        let Some(at) = rest.find("</author>") else {
            break;
        };
        rest = &rest[at + "</author>".len()..];
    }
    record.authors = authors;
    Ok(record)
}

#[derive(Deserialize)]
struct OpenLibraryBook {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    publish_date: Option<String>,
    #[serde(default)]
    publishers: Vec<String>,
    #[serde(default)]
    subjects: Vec<String>,
    /// `[{"key": "/languages/eng"}]`, which is how OpenLibrary names one.
    #[serde(default)]
    languages: Vec<OpenLibraryKey>,
    #[serde(default)]
    number_of_pages: Option<u32>,
}

#[derive(serde::Deserialize)]
struct OpenLibraryKey {
    #[serde(default)]
    key: String,
}

fn open_library(id: &str, kind: &str, body: &str) -> Result<Record, Error> {
    let book: OpenLibraryBook =
        serde_json::from_str(body).map_err(|e| Error::Shape(e.to_string()))?;

    let mut record = blank(id, kind, Source::OpenLibrary);
    // An edition record keeps the subtitle in its own field, and a catalogue
    // wants the whole title. Google Books is already joined the same way.
    record.title = match (book.title, book.subtitle) {
        (Some(title), Some(subtitle)) => Some(format!("{title}: {subtitle}")),
        (title, _) => title,
    };
    record.container = book.publishers.into_iter().next();
    // The date is free text — "1999", "March 1999", "1999-03-01".
    record.year = book.publish_date.as_deref().and_then(year_in);
    record.subjects = book.subjects;
    // "/languages/eng" names the language by the last segment of a key.
    record.language = book
        .languages
        .into_iter()
        .find_map(|l| l.key.rsplit('/').next().map(str::to_string))
        .filter(|l| !l.is_empty());
    record.pages = book.number_of_pages;
    Ok(record)
}

#[derive(Deserialize)]
struct SearchReply {
    #[serde(default)]
    docs: Vec<SearchDoc>,
}

#[derive(Deserialize)]
struct SearchDoc {
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
    language: Vec<String>,
    #[serde(default)]
    subject: Vec<String>,
    #[serde(rename = "number_of_pages_median", default)]
    pages: Option<u32>,
}

/// How many of a work's subject headings are worth carrying.
///
/// The index lists every heading every edition has ever been given, which on a
/// well-known book is hundreds and on Hamlet is thousands. A shelf is filed
/// from a handful, and the first are the broadest.
const SUBJECT_LIMIT: usize = 12;

/// A search that matched nothing answers with no documents rather than a
/// status, so an empty reply is the absence and not a failure.
fn open_library_search(id: &str, kind: &str, body: &str) -> Result<Record, Error> {
    let reply: SearchReply = serde_json::from_str(body).map_err(|e| Error::Shape(e.to_string()))?;
    let Some(doc) = reply.docs.into_iter().next() else {
        return Ok(crate::missing(id, kind, Source::OpenLibrarySearch));
    };

    let mut record = blank(id, kind, Source::OpenLibrarySearch);
    record.title = match (doc.title, doc.subtitle) {
        (Some(title), Some(subtitle)) => Some(format!("{title}: {subtitle}")),
        (title, _) => title,
    };
    record.authors = doc.author_name;
    record.year = doc.first_publish_year;
    record.container = doc.publisher.into_iter().next();
    record.language = doc.language.into_iter().next();
    // The index lists every subject any edition of the work carries, which
    // runs to hundreds on a well-known book. A shelf needs a handful.
    record.subjects = doc.subject;
    record.subjects.truncate(SUBJECT_LIMIT);
    record.pages = doc.pages;
    Ok(record)
}

#[derive(Deserialize)]
struct GoogleReply {
    #[serde(default)]
    items: Vec<GoogleItem>,
}

#[derive(Deserialize)]
struct GoogleItem {
    #[serde(rename = "volumeInfo", default)]
    volume_info: GoogleVolume,
}

#[derive(Deserialize, Default)]
struct GoogleVolume {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(rename = "publishedDate", default)]
    published_date: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(rename = "language", default)]
    language: Option<String>,
    #[serde(rename = "pageCount", default)]
    page_count: Option<u32>,
}

/// A search that matched nothing answers with no items rather than a status,
/// so an empty reply is the absence and not a failure.
fn google_books(id: &str, kind: &str, body: &str) -> Result<Record, Error> {
    let reply: GoogleReply = serde_json::from_str(body).map_err(|e| Error::Shape(e.to_string()))?;
    let Some(item) = reply.items.into_iter().next() else {
        return Ok(crate::missing(id, kind, Source::GoogleBooks));
    };
    let volume = item.volume_info;

    let mut record = blank(id, kind, Source::GoogleBooks);
    record.title = match (volume.title, volume.subtitle) {
        (Some(title), Some(subtitle)) => Some(format!("{title}: {subtitle}")),
        (title, _) => title,
    };
    record.authors = volume.authors;
    record.year = volume.published_date.as_deref().and_then(year_in);
    record.container = volume.publisher;
    record.summary = volume.description.map(|text| collapse(&text));
    record.subjects = volume.categories;
    record.language = volume.language;
    record.pages = volume.page_count;
    Ok(record)
}

/// The first four-digit year in a string.
fn year_in(text: &str) -> Option<i64> {
    let bytes: Vec<char> = text.chars().collect();
    for window in bytes.windows(4) {
        if window.iter().all(char::is_ascii_digit) {
            let year: i64 = window.iter().collect::<String>().parse().ok()?;
            if (1000..=2999).contains(&year) {
                return Some(year);
            }
        }
    }
    None
}

/// What sits between two markers, the first time they appear in order.
fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(&text[start..end])
}

/// Runs of whitespace down to one space, trimmed.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text with its tags removed.
///
/// Crossref hands back JATS — `<jats:p>` and friends — which is markup for a
/// typesetter rather than anything a reader wants.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut inside = false;
    for c in text.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    collapse(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_goes_to_the_body_that_assigns_it() {
        assert_eq!(Source::for_kind("doi"), Some(Source::Crossref));
        assert_eq!(Source::for_kind("arxiv"), Some(Source::Arxiv));
        // The index, not the edition record: it answers with the whole of
        // what a catalogue wants in one request, where the edition record
        // takes up to twenty-eight seconds and names its authors by key.
        assert_eq!(Source::for_kind("isbn"), Some(Source::OpenLibrarySearch));
        // No register answers about an ISSN without a key.
        assert_eq!(Source::for_kind("issn"), None);
    }

    #[test]
    fn a_doi_suffix_cannot_change_the_shape_of_the_url() {
        let url = Source::Crossref.url("10.1000/a?b#c/d");
        assert!(url.ends_with("10.1000%2Fa%3Fb%23c%2Fd"), "{url}");
        assert_eq!(url.matches('?').count(), 0);
    }

    #[test]
    fn crossref_is_read_into_the_fields_that_matter() {
        let body = r#"{"message":{"title":["On Computable Numbers"],
            "author":[{"given":"Alan","family":"Turing"},{"family":"Church"},{"name":"A Committee"}],
            "container-title":["Proceedings"],
            "issued":{"date-parts":[[1936,11,12]]},
            "abstract":"<jats:p>A paragraph.</jats:p>"}}"#;
        let record = crossref("10.1", "doi", body).expect("a record");
        assert!(record.found);
        assert_eq!(record.title.as_deref(), Some("On Computable Numbers"));
        assert_eq!(record.authors, ["Alan Turing", "Church", "A Committee"]);
        assert_eq!(record.year, Some(1936));
        assert_eq!(record.container.as_deref(), Some("Proceedings"));
        assert_eq!(record.summary.as_deref(), Some("A paragraph."));
    }

    #[test]
    fn a_crossref_reply_missing_everything_is_still_a_record() {
        let record = crossref("10.1", "doi", r#"{"message":{}}"#).expect("a record");
        assert!(record.found);
        assert_eq!(record.title, None);
        assert!(record.authors.is_empty());
    }

    #[test]
    fn an_arxiv_feed_with_no_entry_means_no_such_preprint() {
        let record = arxiv("2401.01234", "arxiv", "<feed></feed>").expect("a record");
        assert!(!record.found);
    }

    #[test]
    fn an_arxiv_entry_is_read_out_of_its_atom() {
        let body = "<feed><entry><title>A\n  Title</title>\
            <summary>  Some words.  </summary>\
            <published>2024-01-02T00:00:00Z</published>\
            <author><name>Ada Lovelace</name></author>\
            <author><name>Charles Babbage</name></author></entry></feed>";
        let record = arxiv("2401.01234", "arxiv", body).expect("a record");
        assert!(record.found);
        assert_eq!(record.title.as_deref(), Some("A Title"));
        assert_eq!(record.summary.as_deref(), Some("Some words."));
        assert_eq!(record.year, Some(2024));
        assert_eq!(record.authors, ["Ada Lovelace", "Charles Babbage"]);
    }

    #[test]
    fn openlibrary_gives_up_a_year_from_whatever_it_wrote() {
        for (date, year) in [
            ("1999", Some(1999)),
            ("March 1999", Some(1999)),
            ("1999-03-01", Some(1999)),
            ("n.d.", None),
        ] {
            let body = format!(r#"{{"title":"A Book","publish_date":"{date}"}}"#);
            let record = open_library("1", "isbn", &body).expect("a record");
            assert_eq!(record.year, year, "{date}");
        }
    }

    #[test]
    fn an_answer_that_is_not_what_it_should_be_says_so() {
        assert!(matches!(
            crossref("10.1", "doi", "not json"),
            Err(Error::Shape(_))
        ));
    }

    #[test]
    fn markup_is_not_part_of_an_abstract() {
        assert_eq!(strip_tags("<jats:p>One <i>two</i></jats:p>"), "One two");
        assert_eq!(strip_tags("plain"), "plain");
    }
}
