//! An EPUB's navigation document.
//!
//! Two shapes, because the format changed. EPUB 3 keeps a `<nav>` of nested
//! `<ol>` lists; EPUB 2 keeps an NCX of nested `<navPoint>` elements. Every
//! real book in a library has the second, including most that declare
//! themselves EPUB 3, because converters keep writing it for readers that
//! never caught up.

use std::io::Read;

use crate::{Entry, Missing, number, tidy};

/// How large a navigation document is worth reading.
const MAX_NAV: u64 = 4 * 1024 * 1024;

/// Read an EPUB's table of contents.
pub fn read(bytes: &[u8]) -> Result<Vec<Entry>, Missing> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| Missing::Unreadable(e.to_string()))?;

    let names: Vec<String> = (0..archive.len())
        .filter_map(|at| {
            archive
                .by_index(at)
                .ok()
                .map(|file| file.name().to_string())
        })
        .collect();

    // The NCX first: it is what books actually carry, and its labels are
    // plain text where a nav document's are markup.
    if let Some(name) = names
        .iter()
        .find(|name| name.to_lowercase().ends_with(".ncx"))
    {
        if let Some(text) = member(&mut archive, name) {
            let mut entries = from_ncx(&text);
            if !entries.is_empty() {
                number(&mut entries);
                return Ok(entries);
            }
        }
    }
    for name in names.iter().filter(|name| {
        let lower = name.to_lowercase();
        lower.ends_with("nav.xhtml") || lower.ends_with("nav.html") || lower.ends_with("toc.xhtml")
    }) {
        if let Some(text) = member(&mut archive, name) {
            let mut entries = from_nav(&text);
            if !entries.is_empty() {
                number(&mut entries);
                return Ok(entries);
            }
        }
    }
    Err(Missing::None("navigation document"))
}

fn member<R: Read + std::io::Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let file = archive.by_name(name).ok()?;
    if file.size() > MAX_NAV {
        return None;
    }
    let mut text = String::new();
    file.take(MAX_NAV).read_to_string(&mut text).ok()?;
    Some(text)
}

/// Walk an NCX, where depth is how deeply `<navPoint>` elements nest.
pub fn from_ncx(ncx: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut depth: u32 = 0;
    let mut rest = ncx;

    // Element by element rather than with a parser: the shape is three tags
    // deep and a parser would be the largest dependency in the crate.
    while let Some(at) = rest.find('<') {
        let after = &rest[at..];
        let Some(end) = after.find('>') else { break };
        let tag = &after[1..end];

        if tag.starts_with("navPoint") && !tag.starts_with("navPoint/") {
            let point = &after[end + 1..];
            if let Some(title) = between(point, "<text>", "</text>").map(tidy) {
                if !title.is_empty() {
                    entries.push(Entry {
                        v: corpus_core::V,
                        idx: 0,
                        depth,
                        title,
                        target: between(point, "src=\"", "\"")
                            .or_else(|| between(point, "src='", "'"))
                            .map(|src| src.to_string()),
                        page: None,
                    });
                }
            }
            // A self-closing navPoint has no children and no depth to add.
            if !tag.ends_with('/') {
                depth += 1;
            }
        } else if tag.starts_with("/navPoint") {
            depth = depth.saturating_sub(1);
        }
        rest = &after[end + 1..];
    }
    entries
}

/// Walk an EPUB 3 navigation document, where depth is `<ol>` nesting.
pub fn from_nav(nav: &str) -> Vec<Entry> {
    // Only the table of contents, not the page list or the landmarks.
    let scope = match nav
        .find("epub:type=\"toc\"")
        .or_else(|| nav.find("epub:type='toc'"))
    {
        Some(at) => {
            let from = &nav[at..];
            let end = from.find("</nav>").map_or(from.len(), |end| end + 6);
            &from[..end]
        }
        None => nav,
    };

    let mut entries = Vec::new();
    let mut depth: i64 = -1;
    let mut rest = scope;

    while let Some(at) = rest.find('<') {
        let after = &rest[at..];
        let Some(end) = after.find('>') else { break };
        let tag = &after[1..end];
        let lower = tag.to_lowercase();

        if lower.starts_with("ol") && !lower.starts_with("ol/") {
            depth += 1;
        } else if lower.starts_with("/ol") {
            depth -= 1;
        } else if lower.starts_with("a ") || lower.starts_with("a>") {
            let body = &after[end + 1..];
            if let Some(close) = body.find("</a>") {
                let title = tidy(&strip_tags(&body[..close]));
                if !title.is_empty() {
                    entries.push(Entry {
                        v: corpus_core::V,
                        idx: 0,
                        depth: depth.max(0) as u32,
                        title,
                        target: attribute(tag, "href"),
                        page: None,
                    });
                }
            }
        }
        rest = &after[end + 1..];
    }
    entries
}

/// What sits between two markers.
fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(&text[start..end])
}

/// An attribute out of one tag's text.
fn attribute(tag: &str, name: &str) -> Option<String> {
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

/// Inner text, with the markup a title is wrapped in removed.
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const NCX: &str = r#"<ncx><navMap>
  <navPoint id="a" playOrder="1">
    <navLabel><text>Part One</text></navLabel>
    <content src="part1.xhtml"/>
    <navPoint id="b" playOrder="2">
      <navLabel><text>I. The Setting</text></navLabel>
      <content src="ch1.xhtml"/>
    </navPoint>
    <navPoint id="c" playOrder="3">
      <navLabel><text>II. The Second Estate</text></navLabel>
      <content src="ch2.xhtml"/>
    </navPoint>
  </navPoint>
  <navPoint id="d" playOrder="4">
    <navLabel><text>Part Two</text></navLabel>
    <content src="part2.xhtml"/>
  </navPoint>
</navMap></ncx>"#;

    #[test]
    fn nesting_becomes_depth() {
        let entries = from_ncx(NCX);
        assert_eq!(
            entries
                .iter()
                .map(|e| (e.depth, e.title.as_str()))
                .collect::<Vec<_>>(),
            [
                (0, "Part One"),
                (1, "I. The Setting"),
                (1, "II. The Second Estate"),
                (0, "Part Two"),
            ]
        );
    }

    #[test]
    fn each_entry_keeps_where_it_points() {
        assert_eq!(from_ncx(NCX)[1].target.as_deref(), Some("ch1.xhtml"));
    }

    #[test]
    fn a_nav_document_nests_by_its_lists() {
        let nav = r#"<nav epub:type="toc"><ol>
          <li><a href="p1.xhtml">Part One</a>
            <ol><li><a href="c1.xhtml">I. The Setting</a></li></ol>
          </li>
          <li><a href="p2.xhtml">Part Two</a></li>
        </ol></nav>"#;
        let entries = from_nav(nav);
        assert_eq!(
            entries
                .iter()
                .map(|e| (e.depth, e.title.as_str()))
                .collect::<Vec<_>>(),
            [(0, "Part One"), (1, "I. The Setting"), (0, "Part Two")]
        );
    }

    #[test]
    fn only_the_contents_are_read_and_not_the_page_list() {
        let nav = r#"
          <nav epub:type="toc"><ol><li><a href="c1.xhtml">Chapter One</a></li></ol></nav>
          <nav epub:type="page-list"><ol><li><a href="c1.xhtml#p1">1</a></li></ol></nav>"#;
        let entries = from_nav(nav);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Chapter One");
    }

    #[test]
    fn markup_inside_a_title_is_not_part_of_the_title() {
        let nav = r#"<nav epub:type="toc"><ol><li><a href="c.xhtml">
          <span class="num">1</span> <em>The Setting</em></a></li></ol></nav>"#;
        assert_eq!(from_nav(nav)[0].title, "1 The Setting");
    }

    #[test]
    fn an_entry_with_no_label_is_not_an_entry() {
        let ncx = "<navMap><navPoint><navLabel><text>  </text></navLabel></navPoint></navMap>";
        assert!(from_ncx(ncx).is_empty());
    }

    #[test]
    fn what_is_not_a_zip_says_so() {
        assert!(matches!(read(b"not a zip"), Err(Missing::Unreadable(_))));
    }
}
