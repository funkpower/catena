//! The record a book prints in its own front matter.
//!
//! A book states its own title twice before the text starts, and states it
//! properly: on the title page, set by the publisher, and again in the
//! cataloguing block on the copyright page, where a librarian has already
//! decided what the title is, who the author is, and what the book is about.
//! A file's embedded metadata may say `Untitled` and a filename may say
//! `Oxford`, but the printed page says *The First World War: A Very Short
//! Introduction*, by Michael Howard.
//!
//! So this reads the pages, not the file. What comes back is what the book
//! printed, with the page it was printed on named for every field, and
//! nothing at all where the book printed nothing — a scan that starts at the
//! preface has no title page, and the honest record of that is an empty one.
//!
//! Two traps are worth naming, because both of them are how a reader that
//! does not know better ends up cataloguing the wrong book:
//!
//! - **The series advertisement.** A paperback's front matter lists two
//!   hundred other books in the same series, in the same shape a title page
//!   uses: a work in capitals, a person beside it. A title taken off that
//!   page belongs to somebody else. Those pages are found and refused,
//!   including the ones that continue the list without repeating its heading.
//! - **The uniform title.** A translation's cataloguing block prints the
//!   original title in brackets above the real one. It is a heading, not the
//!   title of the book in your hand.
//!
//! What this does not do is decide whether what the book printed is any
//! good. `THE HUMAN CONDITION` is reported in the capitals it was set in;
//! straightening that is [`bibclean`]'s job. Nor does it look anything up:
//! an ISBN read off the copyright page is a key, and [`bibfetch`] is what
//! turns a key into a catalogue record.
//!
//! [`bibclean`]: https://git.pumpin.online/funkpower/catena
//! [`bibfetch`]: https://git.pumpin.online/funkpower/catena

pub mod cip;
pub mod imprint;
pub mod page;
pub mod prompt;
pub mod shelf;
pub mod title;

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How many pages in from the front to read.
///
/// Front matter that has not finished by here is not front matter. Reading
/// further costs nothing but finds chapter headings, which look enough like
/// titles to be believed.
pub const PAGES: usize = 16;

/// Which page a field was read off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The cataloguing-in-publication block: a catalogue record, printed.
    Cip,
    /// The title page, as the publisher set it.
    TitlePage,
    /// The copyright page, outside the cataloguing block.
    CopyrightPage,
    /// A model, answering about the pages this could not read. Never
    /// written by a read: only [`prompt::merge`] puts it on a record.
    Model,
}

impl Source {
    /// What it is called.
    pub fn name(self) -> &'static str {
        match self {
            Source::Cip => "cip",
            Source::TitlePage => "title_page",
            Source::CopyrightPage => "copyright_page",
            Source::Model => "model",
        }
    }
}

/// What one book printed about itself.
///
/// Every field is written on every record, empty where the book said
/// nothing, and every field that was read names the page it came from in
/// `read_from`. The fields a catalogue needs and this could not find are
/// listed in `unsettled`, so an absence is legible rather than inferred from
/// a missing key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Printed {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// The file this was read from, where it was read from one.
    pub path: Option<String>,
    /// The title, without its subtitle.
    pub title: Option<String>,
    /// The subtitle, where the page set one.
    pub subtitle: Option<String>,
    /// Whoever the book says wrote it, in reading order.
    pub authors: Vec<String>,
    /// Whoever the book says edited it.
    pub editors: Vec<String>,
    /// Checksum-valid ISBNs printed in the front matter, as printed.
    pub isbns: Vec<String>,
    /// The press.
    pub publisher: Option<String>,
    /// Where it publishes from.
    pub place: Option<String>,
    /// The year of publication, preferring first publication.
    pub year: Option<u16>,
    /// The subject headings the cataloguing block filed it under, as printed.
    pub subjects: Vec<String>,
    /// Those headings filed onto a caller's own shelf, when one was given.
    ///
    /// Empty without `--shelf`, and empty too when the book's subjects match
    /// nothing on it — which is an answer, and the reason `unshelved` exists.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shelved: Vec<String>,
    /// Printed headings that matched nothing on the shelf, kept verbatim so a
    /// subject is never silently dropped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unshelved: Vec<String>,
    /// Which page each field was read off.
    pub read_from: BTreeMap<String, Source>,
    /// The fields a catalogue wants that this book did not print.
    pub unsettled: Vec<String>,
    /// How many pages were read.
    pub pages_read: usize,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Printed {
    fn version(&self) -> u32 {
        self.v
    }
}

impl Printed {
    /// Did the book print anything at all?
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.authors.is_empty()
            && self.isbns.is_empty()
            && self.subjects.is_empty()
    }

    fn note(&mut self, field: &str, source: Source) {
        self.read_from.insert(field.to_string(), source);
    }
}

/// Which page holds the title, given where the copyright page is.
///
/// The title page is the last page before the copyright page that is a page
/// of set lines rather than a list or a contents — publishers have put it
/// there for two centuries. Where there is no copyright page to count back
/// from, the first such page before the text starts is all there is to go on.
fn title_page(pages: &[page::Page], copyright: Option<usize>) -> Option<usize> {
    let usable = |i: usize| pages.get(i).is_some_and(|p| p.kind == page::Kind::Title);
    if let Some(c) = copyright {
        if let Some(found) = (0..c).rev().find(|i| usable(*i)) {
            return Some(found);
        }
    }
    pages
        .iter()
        .position(|p| p.kind == page::Kind::Text)
        .map_or_else(
            || (0..pages.len()).find(|i| usable(*i)),
            |end| (0..end).find(|i| usable(*i)),
        )
}

/// The longest printing of the same title anywhere in the front matter.
///
/// A title page breaks its title across lines and sets it in capitals; the
/// half title two leaves earlier prints the whole of it on one line, with
/// its subtitle, in the case the publisher meant. They are the same title,
/// and the unbroken one is the one to keep.
fn longest_printing<'a>(pages: &'a [page::Page], title: &str) -> Option<&'a str> {
    let wanted = squash(title);
    if wanted.len() < 4 {
        return None;
    }
    pages
        .iter()
        .filter(|p| matches!(p.kind, page::Kind::Title | page::Kind::Copyright))
        .flat_map(|p| p.lines.iter())
        .filter(|line| {
            let candidate = squash(line);
            candidate.starts_with(&wanted) && candidate.len() > wanted.len()
        })
        .min_by_key(|line| line.len())
        .map(String::as_str)
}

/// A string reduced to the letters and digits in it, for comparing two
/// printings of one title across a change of case and punctuation.
fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Read a book's front matter, and file its subjects onto a shelf.
///
/// `budget` is how many pages in to read; [`PAGES`] is the sensible default.
/// Passing a [`Shelf`](shelf::Shelf) fills `shelved` and `unshelved`; without
/// one, `subjects` is still read and both stay empty.
pub fn read_onto(text: &str, budget: usize, shelf: Option<&shelf::Shelf>) -> Printed {
    let mut out = read(text, budget);
    if let Some(shelf) = shelf.filter(|s| !s.is_empty()) {
        let filed = shelf.file(&out.subjects);
        if !filed.on.is_empty() {
            out.note("shelved", Source::Cip);
        }
        out.shelved = filed.on;
        out.unshelved = filed.unfiled;
    }
    out
}

/// Read a book's front matter.
///
/// `budget` is how many pages in to read; [`PAGES`] is the sensible default.
pub fn read(text: &str, budget: usize) -> Printed {
    let pages = page::split(text, budget);
    let mut out = Printed {
        v: version(),
        path: None,
        title: None,
        subtitle: None,
        authors: Vec::new(),
        editors: Vec::new(),
        isbns: Vec::new(),
        publisher: None,
        place: None,
        year: None,
        subjects: Vec::new(),
        shelved: Vec::new(),
        unshelved: Vec::new(),
        read_from: BTreeMap::new(),
        unsettled: Vec::new(),
        pages_read: pages.len(),
    };

    let front: String = pages
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let copyright = pages.iter().position(|p| p.kind == page::Kind::Copyright);

    // The cataloguing block is a record somebody else already made, so it
    // outranks the title page wherever the two disagree.
    if let Some(found) = cip::read(&front) {
        if let Some(t) = found.title {
            out.title = Some(t);
            out.note("title", Source::Cip);
        }
        if let Some(s) = found.subtitle {
            out.subtitle = Some(s);
            out.note("subtitle", Source::Cip);
        }
        if !found.authors.is_empty() {
            out.authors = found.authors;
            out.note("authors", Source::Cip);
        }
        if !found.editors.is_empty() {
            out.editors = found.editors;
            out.note("editors", Source::Cip);
        }
        if !found.subjects.is_empty() {
            out.subjects = found.subjects;
            out.note("subjects", Source::Cip);
        }
        out.isbns = found.isbns;
        if !out.isbns.is_empty() {
            out.note("isbns", Source::Cip);
        }
    }

    if let Some(page) = title_page(&pages, copyright).and_then(|i| title::read(&pages[i].lines)) {
        if out.title.is_none() {
            out.title = match page
                .title
                .as_deref()
                .and_then(|t| longest_printing(&pages, t))
            {
                Some(longer) => {
                    let (title, subtitle) = longer
                        .split_once(" : ")
                        .or_else(|| longer.split_once(": "))
                        .map_or((longer.to_string(), None), |(head, tail)| {
                            (head.trim().to_string(), Some(tail.trim().to_string()))
                        });
                    out.subtitle = subtitle;
                    Some(title)
                }
                None => page.title,
            };
            if out.title.is_some() {
                out.note("title", Source::TitlePage);
            }
            if out.subtitle.is_none() && page.subtitle.is_some() {
                out.subtitle = page.subtitle;
                out.note("subtitle", Source::TitlePage);
            }
        }
        if out.authors.is_empty() && !page.authors.is_empty() {
            out.authors = page.authors;
            out.note("authors", Source::TitlePage);
        }
        if out.editors.is_empty() && !page.editors.is_empty() {
            out.editors = page.editors;
            out.note("editors", Source::TitlePage);
        }
    }

    let imprint_text = copyright.map_or(front.as_str(), |i| pages[i].text.as_str());
    let stated = imprint::read(imprint_text);
    for isbn in stated.isbns {
        if !out.isbns.contains(&isbn) {
            out.isbns.push(isbn);
            out.read_from
                .entry("isbns".into())
                .or_insert(Source::CopyrightPage);
        }
    }
    if let Some(publisher) = stated.publisher {
        out.publisher = Some(publisher);
        out.note("publisher", Source::CopyrightPage);
    }
    if let Some(place) = stated.place {
        out.place = Some(place);
        out.note("place", Source::CopyrightPage);
    }
    if let Some(year) = stated.year {
        out.year = Some(year);
        out.note("year", Source::CopyrightPage);
    }

    for (field, missing) in [
        ("title", out.title.is_none()),
        ("authors", out.authors.is_empty() && out.editors.is_empty()),
        ("isbns", out.isbns.is_empty()),
        ("publisher", out.publisher.is_none()),
        ("year", out.year.is_none()),
        ("subjects", out.subjects.is_empty()),
    ] {
        if missing {
            out.unsettled.push(field.to_string());
        }
    }
    out
}
