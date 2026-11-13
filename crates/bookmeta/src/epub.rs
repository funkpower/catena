//! An EPUB's package document, which is where the publisher put it.
//!
//! The archive names a package document in `META-INF/container.xml`, and that
//! document carries Dublin Core elements: title, creator, publisher, date,
//! language, identifier, subject. Calibre adds its own `<meta>` entries for
//! the series, which nothing in the standard has a place for.
//!
//! The cover is whichever image the package names, asked in the order the
//! standards came: EPUB 3's `cover-image` property, EPUB 2's
//! `<meta name="cover">`, the guide's cover page and the image on it, and
//! last any image whose name says cover. Every address is resolved inside the
//! archive and nowhere else, and every member is read no further than a fixed
//! bound, [`MAX_COVER`] for an image.

use std::io::Read;

use crate::{Cover, MAX_COVER, Metadata, Source, image_type};

/// How large a package document is worth reading.
///
/// A real one is a few kilobytes. Anything enormous is either not a package
/// document or is trying to be a decompression bomb.
const MAX_PACKAGE: u64 = 4 * 1024 * 1024;

/// How many members one search for a cover will decompress.
///
/// Each is bounded by [`MAX_COVER`], but a manifest can list as many
/// cover-named images as it likes, and reading every one of them is time a
/// single book is not owed. A real cover is found in one or two reads.
const MAX_READS: usize = 8;

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
    meta.cover = cover(&mut archive, &package_path, &package, &mut meta);
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
        } else if is_uuid(&lower) {
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

/// Whether a value is shaped like a UUID: 36 characters with four hyphens.
pub(crate) fn is_uuid(value: &str) -> bool {
    value.len() == 36 && value.matches('-').count() == 4
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

/// One entry of the package's manifest.
struct Item {
    id: String,
    href: String,
    media_type: String,
    properties: String,
    /// Where `href` leads inside the archive, when it leads anywhere.
    path: Option<String>,
}

/// The cover image, by the first of the four routes that reaches one.
///
/// A route that names something unusable leaves a note and the next route is
/// asked, so a book whose declared cover is missing can still have the one
/// its file name gives away.
fn cover<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    package_path: &str,
    package: &str,
    meta: &mut Metadata,
) -> Option<Cover> {
    let base = directory_of(package_path);
    let items: Vec<Item> = tags(package)
        .filter(|(name, _)| *name == "item")
        .map(|(_, tag)| {
            let href = attr(tag, "href").unwrap_or_default();
            Item {
                id: attr(tag, "id").unwrap_or_default(),
                path: resolve(base, &href),
                href,
                media_type: attr(tag, "media-type").unwrap_or_default(),
                properties: attr(tag, "properties").unwrap_or_default(),
            }
        })
        .collect();
    let mut look = Look {
        archive,
        items: &items,
        tried: Vec::new(),
        reads: 0,
        spent: false,
        meta,
    };

    // EPUB 3: the manifest item whose properties say so.
    for item in items.iter().filter(|item| {
        item.properties
            .split_ascii_whitespace()
            .any(|property| property == "cover-image")
    }) {
        if let Some(found) = look.image(base, &item.href) {
            return Some(found);
        }
    }

    // EPUB 2: `<meta name="cover" content="…">` names an item by its id, or
    // in some converters' hands by its address.
    if let Some(named) = tags(package)
        .filter(|(name, _)| *name == "meta")
        .find(|(_, tag)| attr(tag, "name").as_deref() == Some("cover"))
        .and_then(|(_, tag)| attr(tag, "content"))
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
    {
        let as_path = resolve(base, &named);
        let href = items
            .iter()
            .find(|item| item.id == named)
            .or_else(|| {
                items
                    .iter()
                    .find(|item| item.path.is_some() && item.path == as_path)
            })
            .map_or(named.as_str(), |item| item.href.as_str());
        if let Some(found) = look.image(base, href) {
            return Some(found);
        }
    }

    // The guide's cover page, and the first image on it.
    if let Some(page) = tags(package)
        .filter(|(name, _)| *name == "reference")
        .find(|(_, tag)| attr(tag, "type").is_some_and(|kind| kind.eq_ignore_ascii_case("cover")))
        .and_then(|(_, tag)| attr(tag, "href"))
    {
        if let Some(found) = look.page(base, &page) {
            return Some(found);
        }
    }

    // Last: an image whose id or file name has the word cover in it, one
    // named `cover` outright first, then in manifest order.
    let file_stem = |item: &Item| stem(item.path.as_deref().unwrap_or(&item.href)).to_string();
    let mut named: Vec<&Item> = items
        .iter()
        .filter(|item| {
            item.media_type.starts_with("image/")
                && (says_cover(&item.id) || says_cover(&file_stem(item)))
        })
        .collect();
    named.sort_by_key(|item| !file_stem(item).eq_ignore_ascii_case("cover"));
    for item in named {
        if look.spent {
            break;
        }
        if let Some(mut found) = look.image(base, &item.href) {
            found.guessed = true;
            return Some(found);
        }
    }
    None
}

/// The search for a cover, remembering what it has already looked at so
/// that two routes to the same member read it, and note it, once.
struct Look<'a, R> {
    archive: &'a mut zip::ZipArchive<R>,
    items: &'a [Item],
    tried: Vec<String>,
    /// How many members have been decompressed, against [`MAX_READS`].
    reads: usize,
    /// Whether the reads have run out, and said so.
    spent: bool,
    meta: &'a mut Metadata,
}

impl<R: std::io::Read + std::io::Seek> Look<'_, R> {
    /// Where an address leads, or a note saying it leads nowhere it may.
    fn locate(&mut self, base: &str, href: &str) -> Option<String> {
        if self.spent {
            return None;
        }
        let Some(path) = resolve(base, href) else {
            self.meta.note(format!(
                "the cover's address, {href}, is not a path inside the archive"
            ));
            return None;
        };
        if self.tried.contains(&path) {
            return None;
        }
        self.tried.push(path.clone());
        Some(path)
    }

    /// An image at an address, if that is what is there.
    fn image(&mut self, base: &str, href: &str) -> Option<Cover> {
        let path = self.locate(base, href)?;
        let bytes = self.fetch(&path, MAX_COVER, "cover")?;
        self.check(&path, bytes)
    }

    /// The image a cover page shows, or the page itself where the guide
    /// points straight at an image.
    ///
    /// A page with several images may lead with a logo or an ornament, so
    /// the one whose file name, `alt`, `id` or `class` says cover is taken
    /// over the first; with none of them saying so, the first is taken, and
    /// a note says the page had others.
    fn page(&mut self, base: &str, href: &str) -> Option<Cover> {
        let path = self.locate(base, href)?;
        let bytes = self.fetch(&path, MAX_PACKAGE as usize, "cover page")?;
        if image_type(&bytes).is_some() {
            return self.check(&path, bytes);
        }
        let text = String::from_utf8_lossy(&bytes);
        let images: Vec<(String, bool)> = tags(&text)
            .filter_map(|(name, tag)| {
                let src = match name {
                    "img" => attr(tag, "src"),
                    "image" => attr(tag, "xlink:href").or_else(|| attr(tag, "href")),
                    _ => None,
                }?;
                let named = says_cover(stem(src.split('#').next().unwrap_or("")))
                    || ["alt", "id", "class"]
                        .iter()
                        .any(|name| attr(tag, name).is_some_and(|value| says_cover(&value)));
                Some((src, named))
            })
            .collect();
        let src = if let Some((src, _)) = images.iter().find(|(_, named)| *named) {
            src
        } else if let Some((first, _)) = images.first() {
            if images.len() > 1 {
                self.meta.note(format!(
                    "the cover page, {path}, shows {} images, and the first was taken",
                    images.len()
                ));
            }
            first
        } else {
            self.meta
                .note(format!("the cover page, {path}, shows no image"));
            return None;
        };
        self.image(directory_of(&path), src)
    }

    /// One member's bytes, read no further than `cap`.
    ///
    /// The size the archive declares is checked first and the read is
    /// bounded anyway, because the declaration may be a lie. No more than
    /// [`MAX_READS`] members are read in all; the first asked for after
    /// that ends the search, with a note.
    fn fetch(&mut self, path: &str, cap: usize, what: &str) -> Option<Vec<u8>> {
        if self.spent {
            return None;
        }
        if self.reads == MAX_READS {
            self.spent = true;
            self.meta.note(format!(
                "the search for a cover stopped after reading {MAX_READS} members"
            ));
            return None;
        }
        let file = match self.archive.by_name(path) {
            Ok(file) => file,
            Err(zip::result::ZipError::FileNotFound) => {
                self.meta
                    .note(format!("the {what}, {path}, is not in the archive"));
                return None;
            }
            Err(_) => {
                self.meta
                    .note(format!("the {what}, {path}, could not be read"));
                return None;
            }
        };
        let too_large = format!("the {what}, {path}, is larger than {}", crate::size(cap));
        if file.size() > cap as u64 {
            self.meta.note(too_large);
            return None;
        }
        self.reads += 1;
        let mut bytes = Vec::new();
        if file.take(cap as u64 + 1).read_to_end(&mut bytes).is_err() {
            self.meta
                .note(format!("the {what}, {path}, could not be read"));
            return None;
        }
        if bytes.len() > cap {
            self.meta.note(too_large);
            return None;
        }
        Some(bytes)
    }

    /// The bytes as a cover when they are an image, typed by what they are
    /// rather than by what the manifest says.
    fn check(&mut self, path: &str, bytes: Vec<u8>) -> Option<Cover> {
        let Some(media_type) = image_type(&bytes) else {
            self.meta
                .note(format!("the cover, {path}, is not an image"));
            return None;
        };
        if let Some(item) = self
            .items
            .iter()
            .find(|item| item.path.as_deref() == Some(path))
        {
            let declared = item.media_type.trim();
            if !declared.is_empty() && !declared.eq_ignore_ascii_case(media_type) {
                self.meta.note(format!(
                    "the manifest calls the cover, {path}, {declared}, but it is {media_type}"
                ));
            }
        }
        Some(Cover {
            media_type: media_type.to_string(),
            bytes,
            guessed: false,
        })
    }
}

/// The directory a member sits in, with no trailing slash.
fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// A path's file name without its extension.
fn stem(path: &str) -> &str {
    let name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

/// Whether a name has `cover` in it as a word of its own, in any case.
///
/// Words are split at anything not a letter or a digit and where a lower
/// case letter meets a capital, so `CoverImage`, `Book_COVER` and
/// `cover-thumb` say cover, and `discover` and `recovery` do not.
fn says_cover(name: &str) -> bool {
    words(name)
        .iter()
        .any(|word| word.eq_ignore_ascii_case("cover"))
}

/// The words of a name, as [`says_cover`] splits them.
fn words(name: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut prev = ' ';
    for (at, c) in name.char_indices() {
        let begins =
            c.is_alphanumeric() && (start.is_none() || (prev.is_lowercase() && c.is_uppercase()));
        if !c.is_alphanumeric() || begins {
            if let Some(from) = start.take() {
                out.push(&name[from..at]);
            }
        }
        if begins {
            start = Some(at);
        }
        prev = c;
    }
    if let Some(from) = start {
        out.push(&name[from..]);
    }
    out
}

/// Where an address written in a member in `base` leads inside the archive.
///
/// The fragment comes off and percent-escapes are undone. An address with a
/// scheme, or one that climbs above the root of the archive, leads nowhere
/// this will follow.
fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.split('#').next().unwrap_or("").trim();
    // `http:`, `data:`, `file:`: a scheme before the first slash.
    if let Some(colon) = href.find(':') {
        if !href[..colon].contains('/') {
            return None;
        }
    }
    let href = percent_decode(href)?;
    let mut parts: Vec<&str> = if href.starts_with('/') {
        Vec::new()
    } else {
        base.split('/').filter(|part| !part.is_empty()).collect()
    };
    for part in href.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    // An address of nothing but the directory it is written in names no
    // member at all.
    if href.is_empty() || parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// `%20` back to a space, and so on, where the result is still UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let raw = text.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut at = 0;
    while at < raw.len() {
        if raw[at] == b'%' {
            // Two hex digits and nothing else: `from_str_radix` would also
            // take a sign, and read `%+f` as 15.
            let hex = raw.get(at + 1..at + 3)?;
            if !hex.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            let hex = std::str::from_utf8(hex).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(raw[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Every start tag in a document, in order, as its local name and its text.
///
/// Comments, declarations, processing instructions and end tags are passed
/// over. The local name is what follows any namespace prefix, so
/// `<opf:item>` is an `item`.
fn tags(xml: &str) -> impl Iterator<Item = (&str, &str)> {
    let mut rest = xml;
    std::iter::from_fn(move || {
        loop {
            let at = rest.find('<')?;
            rest = &rest[at..];
            if let Some(after) = rest.strip_prefix("<!--") {
                rest = after.find("-->").map_or("", |end| &after[end + 3..]);
                continue;
            }
            let end = rest.find('>')?;
            let tag = &rest[1..end];
            rest = &rest[end + 1..];
            if tag.starts_with(['/', '!', '?']) {
                continue;
            }
            let name = tag
                .split(|c: char| c.is_ascii_whitespace() || c == '/')
                .next()
                .unwrap_or("");
            let local = name.rsplit_once(':').map_or(name, |(_, local)| local);
            return Some((local, tag));
        }
    })
}

/// The value of one attribute in a tag's text, entities undone.
///
/// The name has to stand on its own — `id` is not the end of `xml:id` or
/// `data-id` — and may have space either side of its `=`.
fn attr(tag: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(found) = tag[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        if !tag[..at].ends_with(|c: char| c.is_ascii_whitespace()) {
            continue;
        }
        let Some(rest) = tag[from..].trim_start().strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
            continue;
        };
        let value = &rest[1..];
        let end = value.find(quote)?;
        return Some(unescape(&value[..end]));
    }
    None
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

    // Covers. Each book is built here, a few members zipped in memory: the
    // container, a package in `OEBPS/`, and whatever the case is about.

    const JPEG: &[u8] = b"\xff\xd8\xff\xe0\x00\x10JFIF\x00jpeg body";
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR png body";
    const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00 gif body";
    const WEBP: &[u8] = b"RIFF\x20\x00\x00\x00WEBPVP8 webp body";

    const CONTAINER: &str = r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    /// A package with this manifest and these extra elements, around a title.
    fn opf(manifest: &str, extra: &str) -> String {
        format!(
            r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Covered</dc:title>
  </metadata>
  <manifest>
    <item id="text" href="text/one.xhtml" media-type="application/xhtml+xml"/>
    {manifest}
  </manifest>
  {extra}
</package>"#
        )
    }

    /// An EPUB of these members, after the container.
    fn epub(members: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let container: (&str, &[u8]) = ("META-INF/container.xml", CONTAINER.as_bytes());
        for (name, data) in std::iter::once(&container).chain(members) {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn book(package: &str, members: &[(&str, &[u8])]) -> Metadata {
        let mut all = vec![("OEBPS/content.opf", package.as_bytes())];
        all.extend_from_slice(members);
        read("x.epub", &epub(&all))
    }

    #[test]
    fn epub_3_names_its_cover_with_a_property() {
        let package = opf(
            r#"<item id="decoy-cover" href="images/cover-thumb.png" media-type="image/png"/>
               <item id="c" href="images/front.jpg" media-type="image/jpeg" properties="svg cover-image"/>"#,
            r#"<metadata><meta name="cover" content="decoy-cover"/></metadata>"#,
        );
        let meta = book(
            &package,
            &[
                ("OEBPS/images/front.jpg", JPEG),
                ("OEBPS/images/cover-thumb.png", PNG),
            ],
        );
        let cover = meta.cover.expect("a cover");
        assert_eq!(cover.media_type, "image/jpeg");
        assert_eq!(cover.bytes, JPEG);
        assert!(!cover.guessed, "a declared cover is not a guess");
        assert_eq!(cover.extension(), "jpg");
        assert!(meta.notes.is_empty(), "{:?}", meta.notes);
    }

    #[test]
    fn epub_2_names_its_cover_with_a_meta_holding_an_id() {
        let package = opf(
            r#"<opf:item id="cover-img" href="Images/Front%20Page.png#top" media-type="image/png"/>"#,
            r#"<metadata><meta content="cover-img" name="cover" /></metadata>"#,
        );
        let meta = book(&package, &[("OEBPS/Images/Front Page.png", PNG)]);
        let cover = meta.cover.expect("a cover");
        assert_eq!(cover.media_type, "image/png");
        assert_eq!(cover.bytes, PNG);
        assert!(meta.notes.is_empty(), "{:?}", meta.notes);
    }

    #[test]
    fn a_cover_meta_holding_an_address_is_followed_as_one() {
        // Listed in the manifest under another id, and not listed at all.
        let listed = opf(
            r#"<item id="x1" href="../art/front.gif" media-type="image/gif"/>"#,
            r#"<metadata><meta name="cover" content="../art/front.gif"/></metadata>"#,
        );
        let meta = book(&listed, &[("art/front.gif", GIF)]);
        assert_eq!(meta.cover.expect("a cover").media_type, "image/gif");

        let unlisted = opf(
            "",
            r#"<metadata><meta name="cover" content="front.webp"/></metadata>"#,
        );
        let meta = book(&unlisted, &[("OEBPS/front.webp", WEBP)]);
        let cover = meta.cover.expect("a cover");
        assert_eq!(cover.media_type, "image/webp");
        assert_eq!(cover.bytes, WEBP);
    }

    #[test]
    fn the_guide_cover_page_gives_the_first_image_on_it() {
        let package = opf(
            r#"<item id="page" href="text/cover.xhtml" media-type="application/xhtml+xml"/>
               <item id="art" href="images/art.jpg" media-type="image/jpeg"/>"#,
            r#"<guide><reference type="Cover" title="Cover" href="text/cover.xhtml#start"/></guide>"#,
        );
        let page = br#"<html><body><!-- <img src="nothing.png"/> -->
            <img alt="cover" src="../images/art.jpg"/><img src="../images/later.png"/></body></html>"#;
        let meta = book(
            &package,
            &[
                ("OEBPS/text/cover.xhtml", page),
                ("OEBPS/images/art.jpg", JPEG),
                ("OEBPS/images/later.png", PNG),
            ],
        );
        let cover = meta.cover.expect("a cover");
        assert_eq!(cover.bytes, JPEG);
        assert!(meta.notes.is_empty(), "{:?}", meta.notes);
    }

    #[test]
    fn the_guide_cover_page_may_draw_its_image_in_svg() {
        let package = opf(
            "",
            r#"<guide><reference type="cover" href="titlepage.xhtml"/></guide>"#,
        );
        for image in [
            r#"<svg:image width="600" xlink:href="cover.png"/>"#,
            r#"<image href="cover.png" />"#,
        ] {
            let page = format!(r#"<html><body><svg:svg>{image}</svg:svg></body></html>"#);
            let meta = book(
                &package,
                &[
                    ("OEBPS/titlepage.xhtml", page.as_bytes()),
                    ("OEBPS/cover.png", PNG),
                ],
            );
            assert_eq!(meta.cover.expect(image).bytes, PNG);
        }
    }

    #[test]
    fn last_an_image_whose_name_says_cover() {
        let package = opf(
            r#"<item id="pic1" href="images/plate.jpg" media-type="image/jpeg"/>
               <item id="CoverImage" href="images/i2.gif" media-type="image/gif"/>"#,
            "",
        );
        let meta = book(
            &package,
            &[
                ("OEBPS/images/plate.jpg", JPEG),
                ("OEBPS/images/i2.gif", GIF),
            ],
        );
        let cover = meta.cover.expect("a cover");
        assert_eq!(cover.bytes, GIF);
        assert!(cover.guessed, "a cover found by its name is a guess");
        assert_eq!(cover.extension(), "gif");

        let by_address = opf(
            r#"<item id="i9" href="images/Book_COVER.jpeg" media-type="image/jpeg"/>"#,
            "",
        );
        let meta = book(&by_address, &[("OEBPS/images/Book_COVER.jpeg", JPEG)]);
        assert_eq!(meta.cover.expect("a cover").bytes, JPEG);
    }

    #[test]
    fn a_name_says_cover_only_as_a_word_of_its_own() {
        for yes in [
            "cover",
            "CoverImage",
            "Book_COVER",
            "cover-thumb",
            "frontCover",
            "x.cover",
        ] {
            assert!(says_cover(yes), "{yes}");
        }
        for no in [
            "discover-ch1",
            "recovery-map",
            "covers",
            "Coverage",
            "uncover",
            "",
        ] {
            assert!(!says_cover(no), "{no}");
        }
        assert_eq!(words("CoverImage_v2"), ["Cover", "Image", "v2"]);
    }

    #[test]
    fn a_name_that_only_contains_cover_is_not_a_cover() {
        let package = opf(
            r#"<item id="ch1" href="images/discover-ch1.png" media-type="image/png"/>
               <item id="i2" href="images/cover.jpg" media-type="image/jpeg"/>"#,
            "",
        );
        let meta = book(
            &package,
            &[
                ("OEBPS/images/discover-ch1.png", PNG),
                ("OEBPS/images/cover.jpg", JPEG),
            ],
        );
        let cover = meta.cover.expect("a cover");
        assert_eq!(cover.bytes, JPEG);
        assert!(cover.guessed);

        let alone = opf(
            r#"<item id="map" href="images/recovery-map.png" media-type="image/png"/>"#,
            "",
        );
        let meta = book(&alone, &[("OEBPS/images/recovery-map.png", PNG)]);
        assert_eq!(meta.cover, None);
        assert!(meta.notes.is_empty(), "{:?}", meta.notes);
    }

    #[test]
    fn an_image_named_cover_outright_beats_one_listed_before_it() {
        let package = opf(
            r#"<item id="cover-thumb" href="images/thumb.png" media-type="image/png"/>
               <item id="i2" href="images/Cover.jpg" media-type="image/jpeg"/>"#,
            "",
        );
        let meta = book(
            &package,
            &[
                ("OEBPS/images/thumb.png", PNG),
                ("OEBPS/images/Cover.jpg", JPEG),
            ],
        );
        let cover = meta.cover.expect("a cover");
        assert_eq!(cover.bytes, JPEG);
        assert!(cover.guessed);
    }

    #[test]
    fn the_guide_cover_page_prefers_the_image_that_says_cover() {
        let package = opf(
            "",
            r#"<guide><reference type="cover" href="text/cover.xhtml"/></guide>"#,
        );
        for art in [
            r#"<img src="../images/front-cover.jpg"/>"#,
            r#"<img alt="Cover" src="../images/front-cover.jpg"/>"#,
            r#"<img id="coverImage" src="../images/front-cover.jpg"/>"#,
            r#"<img class="full cover" src="../images/front-cover.jpg"/>"#,
        ] {
            let page = format!(r#"<html><body><img src="../images/logo.png"/>{art}</body></html>"#);
            let meta = book(
                &package,
                &[
                    ("OEBPS/text/cover.xhtml", page.as_bytes()),
                    ("OEBPS/images/logo.png", PNG),
                    ("OEBPS/images/front-cover.jpg", JPEG),
                ],
            );
            assert_eq!(meta.cover.expect(art).bytes, JPEG, "{art}");
            assert!(meta.notes.is_empty(), "{art}: {:?}", meta.notes);
        }
    }

    #[test]
    fn a_guide_cover_page_with_several_unnamed_images_takes_the_first_and_says_so() {
        let package = opf(
            "",
            r#"<guide><reference type="cover" href="text/cover.xhtml"/></guide>"#,
        );
        let page = br#"<html><body><img src="../images/logo.png"/><img src="../images/art.jpg"/></body></html>"#;
        let meta = book(
            &package,
            &[
                ("OEBPS/text/cover.xhtml", page),
                ("OEBPS/images/logo.png", PNG),
                ("OEBPS/images/art.jpg", JPEG),
            ],
        );
        assert_eq!(meta.cover.expect("a cover").bytes, PNG);
        assert_eq!(
            meta.notes,
            ["the cover page, OEBPS/text/cover.xhtml, shows 2 images, and the first was taken"]
        );
    }

    #[test]
    fn a_search_for_a_cover_reads_no_more_than_its_budget() {
        let count = MAX_READS + 4;
        let manifest: String = (0..count)
            .map(|n| format!(r#"<item id="cover-{n}" href="c{n}.jpg" media-type="image/jpeg"/>"#))
            .collect();
        let names: Vec<String> = (0..count).map(|n| format!("OEBPS/c{n}.jpg")).collect();
        let members: Vec<(&str, &[u8])> = names
            .iter()
            .map(|name| (name.as_str(), &b"<html>not an image</html>"[..]))
            .collect();
        let meta = book(&opf(&manifest, ""), &members);
        assert_eq!(meta.cover, None);
        let mut expected: Vec<String> = (0..MAX_READS)
            .map(|n| format!("the cover, OEBPS/c{n}.jpg, is not an image"))
            .collect();
        expected.push(format!(
            "the search for a cover stopped after reading {MAX_READS} members"
        ));
        assert_eq!(meta.notes, expected);
    }

    #[test]
    fn a_book_that_names_no_cover_has_none_and_says_nothing() {
        let package = opf(
            r#"<item id="plate" href="images/plate.jpg" media-type="image/jpeg"/>"#,
            "",
        );
        let meta = book(&package, &[("OEBPS/images/plate.jpg", JPEG)]);
        assert_eq!(meta.cover, None);
        assert!(meta.notes.is_empty(), "{:?}", meta.notes);
    }

    #[test]
    fn a_mislabelled_cover_is_what_its_bytes_say() {
        let package = opf(
            r#"<item id="c" href="c.png" media-type="image/png" properties="cover-image"/>"#,
            "",
        );
        let meta = book(&package, &[("OEBPS/c.png", JPEG)]);
        assert_eq!(meta.cover.expect("a cover").media_type, "image/jpeg");
        assert_eq!(
            meta.notes,
            ["the manifest calls the cover, OEBPS/c.png, image/png, but it is image/jpeg"]
        );
    }

    #[test]
    fn a_cover_that_is_not_an_image_is_a_note_and_not_a_cover() {
        let package = opf(
            r#"<item id="c" href="c.jpg" media-type="image/jpeg" properties="cover-image"/>"#,
            "",
        );
        let meta = book(&package, &[("OEBPS/c.jpg", b"<html>not an image</html>")]);
        assert_eq!(meta.cover, None);
        assert_eq!(meta.notes, ["the cover, OEBPS/c.jpg, is not an image"]);
    }

    #[test]
    fn a_cover_larger_than_the_cap_is_left_out() {
        let mut huge = JPEG.to_vec();
        huge.resize(MAX_COVER + 1, 0);
        let package = opf(
            r#"<item id="c" href="c.jpg" media-type="image/jpeg" properties="cover-image"/>"#,
            "",
        );
        let meta = book(&package, &[("OEBPS/c.jpg", &huge)]);
        assert_eq!(meta.cover, None);
        assert_eq!(
            meta.notes,
            ["the cover, OEBPS/c.jpg, is larger than 16 MiB"]
        );
    }

    /// Read one member through `Look::fetch` with a cap of `cap`.
    fn fetch(bytes: Vec<u8>, cap: usize) -> (Option<usize>, Vec<String>) {
        let mut meta = Metadata::empty("x.epub", Source::Opf);
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut look = Look {
            archive: &mut archive,
            items: &[],
            tried: Vec::new(),
            reads: 0,
            spent: false,
            meta: &mut meta,
        };
        let got = look.fetch("big.jpg", cap, "cover").map(|bytes| bytes.len());
        (got, meta.notes)
    }

    #[test]
    fn a_member_is_read_no_further_than_its_cap() {
        let bytes = epub(&[("big.jpg", &[0u8; 4096])]);
        assert_eq!(fetch(bytes.clone(), 4096).0, Some(4096));
        assert_eq!(
            fetch(bytes, 4095),
            (
                None,
                vec!["the cover, big.jpg, is larger than 4095 bytes".to_string()]
            )
        );
    }

    #[test]
    fn a_member_whose_archive_understates_its_size_is_still_bounded() {
        let mut bytes = epub(&[("big.jpg", &[0u8; 4096])]);
        // Declare 10 bytes uncompressed, in the local header and in the
        // central directory, where 4096 are compressed in.
        // The name comes 30 bytes into a local header and 46 into a central
        // directory entry, and the uncompressed size 22 and 24.
        let local = find(&bytes, b"big.jpg") - 30;
        bytes[local + 22..local + 26].copy_from_slice(&10u32.to_le_bytes());
        let central = local + 31 + find(&bytes[local + 31..], b"big.jpg") - 46;
        assert_eq!(&bytes[central..central + 4], b"PK\x01\x02");
        bytes[central + 24..central + 28].copy_from_slice(&10u32.to_le_bytes());

        // The declaration passes the check, and the read stops at the cap.
        let (got, notes) = fetch(bytes, 100);
        assert_eq!(got, None);
        assert_eq!(notes, ["the cover, big.jpg, is larger than 100 bytes"]);
    }

    fn find(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("present")
    }

    #[test]
    fn a_cover_named_but_missing_is_a_note_and_the_next_route_is_asked() {
        let package = opf(
            r#"<item id="c" href="gone.jpg" media-type="image/jpeg" properties="cover-image"/>"#,
            "",
        );
        let meta = book(&package, &[]);
        assert_eq!(meta.cover, None);
        assert_eq!(
            meta.notes,
            ["the cover, OEBPS/gone.jpg, is not in the archive"]
        );

        // A missing page is said the same way.
        let page = opf(
            "",
            r#"<guide><reference type="cover" href="none.xhtml"/></guide>"#,
        );
        assert_eq!(
            book(&page, &[]).notes,
            ["the cover page, OEBPS/none.xhtml, is not in the archive"]
        );

        // The same file reached again by another route is not read twice,
        // and an image the name gives away still stands in.
        let package = opf(
            r#"<item id="cover" href="gone.jpg" media-type="image/jpeg" properties="cover-image"/>
               <item id="cover-small" href="small.png" media-type="image/png"/>"#,
            r#"<metadata><meta name="cover" content="cover"/></metadata>"#,
        );
        let meta = book(&package, &[("OEBPS/small.png", PNG)]);
        assert_eq!(meta.cover.expect("the stand-in").bytes, PNG);
        assert_eq!(
            meta.notes,
            ["the cover, OEBPS/gone.jpg, is not in the archive"]
        );
    }

    #[test]
    fn an_address_never_leaves_the_archive() {
        assert_eq!(resolve("OEBPS", "../c.jpg").as_deref(), Some("c.jpg"));
        assert_eq!(resolve("OEBPS", "/c.jpg").as_deref(), Some("c.jpg"));
        assert_eq!(
            resolve("OEBPS/text", "./a/../b.jpg").as_deref(),
            Some("OEBPS/text/b.jpg")
        );
        assert_eq!(resolve("", "a%2Fb.jpg").as_deref(), Some("a/b.jpg"));
        assert_eq!(percent_decode("a%2fb%2F").as_deref(), Some("a/b/"));
        assert_eq!(percent_decode("a%+fb"), None);
        for hostile in [
            "../../c.jpg",
            "../../../etc/passwd",
            "http://example.com/c.jpg",
            "data:image/png;base64,AAAA",
            "file:///etc/passwd",
            "%ZZ.jpg",
            "%ff.jpg",
            "a%+fb",
            "a%-1b",
            "a% fb",
            "c%",
            "#only-a-fragment",
            "",
        ] {
            assert_eq!(resolve("OEBPS", hostile), None, "{hostile}");
        }

        let package = opf(
            r#"<item id="c" href="../../c.jpg" media-type="image/jpeg" properties="cover-image"/>"#,
            "",
        );
        let meta = book(&package, &[("c.jpg", JPEG)]);
        assert_eq!(meta.cover, None);
        assert_eq!(
            meta.notes,
            ["the cover's address, ../../c.jpg, is not a path inside the archive"]
        );
    }

    #[test]
    fn an_attribute_is_matched_by_its_whole_name() {
        let tag = r#"item data-id="no" xml:id="no" id = 'yes' href="a&amp;b.jpg""#;
        assert_eq!(attr(tag, "id").as_deref(), Some("yes"));
        assert_eq!(attr(tag, "href").as_deref(), Some("a&b.jpg"));
        assert_eq!(attr(r#"image xlink:href="x.png""#, "href"), None);
        assert_eq!(attr(r#"item id="unterminated"#, "id"), None);
    }

    #[test]
    fn every_truncation_of_a_book_with_a_cover_returns() {
        let package = opf(
            r#"<item id="c" href="c.jpg" media-type="image/jpeg" properties="cover-image"/>"#,
            r#"<guide><reference type="cover" href="p.xhtml"/></guide>"#,
        );
        let bytes = epub(&[
            ("OEBPS/content.opf", package.as_bytes()),
            ("OEBPS/c.jpg", JPEG),
            ("OEBPS/p.xhtml", b"<img src=\"c.jpg\"/>"),
        ]);
        for len in 0..=bytes.len() {
            let _ = read("cut.epub", &bytes[..len]);
        }
        for at in 0..bytes.len() {
            let mut broken = bytes.clone();
            broken[at] ^= 0xff;
            let _ = read("broken.epub", &broken);
        }
    }
}
