//! An EPUB's package document, which is where the publisher put it.
//!
//! The archive names a package document in `META-INF/container.xml`, and that
//! document carries Dublin Core elements: title, creator, publisher, date,
//! language, identifier, subject. Calibre adds its own `<meta>` entries for
//! the series, which nothing in the standard has a place for.

use std::io::Read;

use crate::{Metadata, Source};

/// How large a package document is worth reading.
///
/// A real one is a few kilobytes. Anything enormous is either not a package
/// document or is trying to be a decompression bomb.
const MAX_PACKAGE: u64 = 4 * 1024 * 1024;

/// Read what an EPUB says about itself.
pub fn read(path: &str, bytes: &[u8]) -> Metadata {
    let mut meta = Metadata::empty(path, Source::Opf);

    let cursor = std::io::Cursor::new(bytes);
    let Ok(mut archive) = zip::ZipArchive::new(cursor) else {
        meta.source = Source::None.name().to_string();
        meta.note("a zip whose directory could not be read");
        return meta;
    };

    let Some(package_path) = package_path(&mut archive) else {
        meta.source = Source::None.name().to_string();
        meta.note("no package document, so not an EPUB");
        return meta;
    };
    let Some(package) = member(&mut archive, &package_path) else {
        meta.source = Source::None.name().to_string();
        meta.note(format!("{package_path} could not be read"));
        return meta;
    };

    fill(&mut meta, &package);
    if meta.is_empty() {
        meta.note("a package document with nothing filled in");
    }
    meta
}

/// Where the package document lives, per `META-INF/container.xml`.
///
/// Falls back to the first `.opf` in the archive, because a converter that
/// wrote a malformed container often still wrote a usable package.
fn package_path<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Option<String> {
    if let Some(container) = member(archive, "META-INF/container.xml") {
        if let Some(found) = attribute(&container, "rootfile", "full-path") {
            return Some(found);
        }
    }
    (0..archive.len())
        .filter_map(|at| {
            archive
                .by_index(at)
                .ok()
                .map(|file| file.name().to_string())
        })
        .find(|name| name.to_lowercase().ends_with(".opf"))
}

/// One member's text, where it is small enough to be worth reading.
fn member<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let file = archive.by_name(name).ok()?;
    if file.size() > MAX_PACKAGE {
        return None;
    }
    let mut text = String::new();
    file.take(MAX_PACKAGE).read_to_string(&mut text).ok()?;
    Some(text)
}

/// Fill a record from a package document.
fn fill(meta: &mut Metadata, package: &str) {
    meta.title = first(package, "title");
    meta.authors = creators(package);
    meta.publisher = first(package, "publisher");
    meta.description = first(package, "description");
    meta.date = first(package, "date");
    meta.language = first(package, "language");
    meta.subjects = all(package, "subject");

    for (scheme, value) in identifiers(package) {
        meta.identifiers.insert(scheme, value);
    }
    meta.series = meta_content(package, "calibre:series");
    meta.series_index = meta_content(package, "calibre:series_index");
}

/// The text of the first `dc:<name>` element, namespace prefix or not.
fn first(package: &str, name: &str) -> Option<String> {
    all(package, name).into_iter().next()
}

/// The text of every `dc:<name>` element.
fn all(package: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for prefix in ["dc:", ""] {
        let open = format!("<{prefix}{name}");
        let close = format!("</{prefix}{name}>");
        let mut rest = package;
        while let Some(at) = rest.find(&open) {
            let after = &rest[at + open.len()..];
            // `<title` must not match `<titlepage`.
            if !after.starts_with(['>', ' ', '\t', '\n', '\r', '/']) {
                rest = after;
                continue;
            }
            let Some(gt) = after.find('>') else { break };
            let Some(end) = after[gt + 1..].find(&close) else {
                break;
            };
            let text = unescape(after[gt + 1..gt + 1 + end].trim());
            if !text.is_empty() {
                out.push(text);
            }
            rest = &after[gt + 1 + end..];
        }
        if !out.is_empty() {
            return out;
        }
    }
    out
}

/// The people credited, with `file-as` spellings left out.
///
/// A package lists `<dc:creator>` for authors and `<dc:contributor>` for
/// everyone else; only the first is a credit a catalogue wants.
fn creators(package: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in all(package, "creator") {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Every identifier, by scheme.
///
/// The scheme is in an `opf:scheme` attribute where the publisher bothered,
/// and otherwise has to be read off the value: `urn:isbn:…`, a bare ISBN, a
/// UUID.
fn identifiers(package: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for raw in all(package, "identifier") {
        let value = raw.trim();
        let lower = value.to_lowercase();
        let (scheme, id) = if let Some(rest) = lower.strip_prefix("urn:isbn:") {
            ("isbn", rest.to_string())
        } else if let Some(rest) = lower.strip_prefix("urn:uuid:") {
            ("uuid", rest.to_string())
        } else if let Some(rest) = lower.strip_prefix("urn:doi:") {
            ("doi", rest.to_string())
        } else if lower.starts_with("10.") && lower.contains('/') {
            ("doi", value.to_string())
        } else if is_isbn(value) {
            ("isbn", digits_of(value))
        } else if lower.len() == 36 && lower.matches('-').count() == 4 {
            ("uuid", lower.clone())
        } else {
            ("other", value.to_string())
        };
        if !id.is_empty() {
            out.push((scheme.to_string(), id));
        }
    }
    out
}

/// Whether a string is ten or thirteen digits once its hyphens come off.
pub(crate) fn is_isbn(value: &str) -> bool {
    let digits = digits_of(value);
    matches!(digits.len(), 10 | 13)
        && value
            .chars()
            .all(|c| c.is_ascii_digit() || "- xX".contains(c))
}

/// The digits of an ISBN, with its check character, and nothing else.
pub(crate) fn digits_of(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, 'x' | 'X'))
        .collect()
}

/// The `content` of a `<meta name="…">`, which is where Calibre puts a series.
fn meta_content(package: &str, name: &str) -> Option<String> {
    let mut rest = package;
    while let Some(at) = rest.find("<meta") {
        let after = &rest[at..];
        let end = after.find('>')?;
        let tag = &after[..end];
        if tag.contains(&format!("\"{name}\"")) || tag.contains(&format!("'{name}'")) {
            if let Some(content) = attribute_in(tag, "content") {
                return Some(unescape(&content));
            }
        }
        rest = &after[end..];
    }
    None
}

/// An attribute of the first element with a given name.
fn attribute(xml: &str, element: &str, name: &str) -> Option<String> {
    let at = xml.find(&format!("<{element}"))?;
    let end = xml[at..].find('>')? + at;
    attribute_in(&xml[at..end], name)
}

/// An attribute out of one tag's text.
fn attribute_in(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(at) = tag.find(&needle) {
            let rest = &tag[at + needle.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

/// The five XML entities, which is all a package document uses.
fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACKAGE: &str = r#"<?xml version="1.0"?>
<package xmlns:dc="http://purl.org/dc/elements/1.1/" version="3.0">
  <metadata>
    <dc:title>The Philokalia, Volume 3</dc:title>
    <dc:creator opf:role="aut">G. E. H. Palmer</dc:creator>
    <dc:creator opf:role="aut">Kallistos Ware</dc:creator>
    <dc:contributor>A Typesetter</dc:contributor>
    <dc:publisher>Faber &amp; Faber</dc:publisher>
    <dc:description>&lt;p&gt;Writings of the Fathers.&lt;/p&gt;</dc:description>
    <dc:date>1984-09-17</dc:date>
    <dc:language>en</dc:language>
    <dc:subject>Orthodox Christianity</dc:subject>
    <dc:subject>Asceticism</dc:subject>
    <dc:identifier opf:scheme="ISBN">978-0-571-17525-6</dc:identifier>
    <dc:identifier>urn:uuid:8f1a2b3c-4d5e-6f70-8192-a3b4c5d6e7f8</dc:identifier>
    <meta name="calibre:series" content="The Philokalia"/>
    <meta name="calibre:series_index" content="3"/>
  </metadata>
</package>"#;

    fn read_package() -> Metadata {
        let mut meta = Metadata::empty("x.epub", Source::Opf);
        fill(&mut meta, PACKAGE);
        meta
    }

    #[test]
    fn the_dublin_core_elements_come_out() {
        let meta = read_package();
        assert_eq!(meta.title.as_deref(), Some("The Philokalia, Volume 3"));
        assert_eq!(meta.publisher.as_deref(), Some("Faber & Faber"));
        assert_eq!(
            meta.description.as_deref(),
            Some("<p>Writings of the Fathers.</p>")
        );
        assert_eq!(meta.date.as_deref(), Some("1984-09-17"));
        assert_eq!(meta.language.as_deref(), Some("en"));
    }

    #[test]
    fn every_author_is_kept_in_order_and_contributors_are_not_authors() {
        let meta = read_package();
        assert_eq!(meta.authors, ["G. E. H. Palmer", "Kallistos Ware"]);
    }

    #[test]
    fn identifiers_come_back_under_the_scheme_they_are() {
        let meta = read_package();
        assert_eq!(
            meta.identifiers.get("isbn").map(String::as_str),
            Some("9780571175256")
        );
        assert_eq!(
            meta.identifiers.get("uuid").map(String::as_str),
            Some("8f1a2b3c-4d5e-6f70-8192-a3b4c5d6e7f8")
        );
    }

    #[test]
    fn a_series_is_read_from_the_meta_entries_calibre_adds() {
        let meta = read_package();
        assert_eq!(meta.series.as_deref(), Some("The Philokalia"));
        assert_eq!(meta.series_index.as_deref(), Some("3"));
    }

    #[test]
    fn subjects_come_out_in_order() {
        assert_eq!(
            read_package().subjects,
            ["Orthodox Christianity", "Asceticism"]
        );
    }

    #[test]
    fn entities_are_undone_once_and_not_twice() {
        // `&amp;lt;` is a literal "&lt;", not a "<".
        assert_eq!(unescape("Faber &amp; Faber"), "Faber & Faber");
        assert_eq!(unescape("a &amp;lt; b"), "a &lt; b");
    }

    #[test]
    fn a_package_without_the_dublin_core_prefix_still_reads() {
        let bare =
            "<package><metadata><title>Bare</title><creator>Nobody</creator></metadata></package>";
        let mut meta = Metadata::empty("x.epub", Source::Opf);
        fill(&mut meta, bare);
        assert_eq!(meta.title.as_deref(), Some("Bare"));
        assert_eq!(meta.authors, ["Nobody"]);
    }

    #[test]
    fn an_element_that_merely_starts_the_same_way_is_not_the_element() {
        let tricky = "<package><titlepage>no</titlepage><dc:title>Yes</dc:title></package>";
        let mut meta = Metadata::empty("x.epub", Source::Opf);
        fill(&mut meta, tricky);
        assert_eq!(meta.title.as_deref(), Some("Yes"));
    }

    #[test]
    fn what_is_not_a_zip_says_so_rather_than_guessing() {
        let meta = read("x.epub", b"not a zip at all");
        assert_eq!(meta.source, "none");
        assert!(!meta.notes.is_empty());
    }
}
