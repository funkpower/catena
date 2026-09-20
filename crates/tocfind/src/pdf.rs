//! A PDF's outline, which is what a reader shows in its sidebar.
//!
//! The catalogue names an outline whose items nest, each with a title and a
//! destination. Resolving a destination to a page number means finding which
//! page object it names, which is worth doing: a chapter that knows its page
//! can be cited, and one that knows only an object number cannot.

use lopdf::{Document, Object, ObjectId};

use crate::{Entry, Missing, number, tidy};

/// How deep an outline is followed before something is assumed to be wrong.
const MAX_DEPTH: u32 = 16;

/// How many items are read before stopping.
///
/// A real outline is hundreds; a malformed one that loops is unbounded, and
/// the depth cap alone does not catch a cycle among siblings.
const MAX_ITEMS: usize = 10_000;

/// Read a PDF's outline.
pub fn read(bytes: &[u8]) -> Result<Vec<Entry>, Missing> {
    let document = Document::load_mem(bytes).map_err(|e| Missing::Unreadable(e.to_string()))?;
    if document.is_encrypted() {
        return Err(Missing::Unreadable("it is encrypted".into()));
    }

    let pages: Vec<ObjectId> = document.get_pages().into_values().collect();
    let Some(root) = outline_root(&document) else {
        return Err(Missing::None("outline"));
    };

    let mut entries = Vec::new();
    let mut seen: Vec<ObjectId> = Vec::new();
    walk(&document, &pages, root, 0, &mut entries, &mut seen);

    match entries.is_empty() {
        true => Err(Missing::None("outline entries")),
        false => {
            number(&mut entries);
            Ok(entries)
        }
    }
}

/// The first item of the catalogue's outline.
fn outline_root(document: &Document) -> Option<ObjectId> {
    let outlines = document.catalog().ok()?.get(b"Outlines").ok()?;
    let (_, object) = document.dereference(outlines).ok()?;
    object
        .as_dict()
        .ok()?
        .get(b"First")
        .ok()?
        .as_reference()
        .ok()
}

/// Follow a chain of siblings, descending into each one's children.
fn walk(
    document: &Document,
    pages: &[ObjectId],
    first: ObjectId,
    depth: u32,
    entries: &mut Vec<Entry>,
    seen: &mut Vec<ObjectId>,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let mut at = Some(first);
    while let Some(id) = at {
        if entries.len() >= MAX_ITEMS || seen.contains(&id) {
            return;
        }
        seen.push(id);

        let Ok(item) = document.get_dictionary(id) else {
            return;
        };
        if let Some(title) = item.get(b"Title").ok().and_then(|t| text(document, t)) {
            let title = tidy(&title);
            if !title.is_empty() {
                entries.push(Entry {
                    v: corpus_core::V,
                    idx: 0,
                    depth,
                    title,
                    target: None,
                    page: page_of(document, pages, item),
                });
            }
        }

        if let Some(child) = item.get(b"First").ok().and_then(|f| f.as_reference().ok()) {
            walk(document, pages, child, depth + 1, entries, seen);
        }
        at = item
            .get(b"Next")
            .ok()
            .and_then(|next| next.as_reference().ok());
    }
}

/// Which page an item points at, counting from one.
///
/// A destination is either an array beginning with a page reference or a
/// named one this does not chase, because a name table is a second index and
/// an entry without a page is still an entry.
fn page_of(document: &Document, pages: &[ObjectId], item: &lopdf::Dictionary) -> Option<u32> {
    let destination = match item.get(b"Dest") {
        Ok(dest) => dest.clone(),
        Err(_) => {
            // An action rather than a destination: `/A << /D [...] >>`.
            let action = item.get(b"A").ok()?;
            let (_, action) = document.dereference(action).ok()?;
            action.as_dict().ok()?.get(b"D").ok()?.clone()
        }
    };
    let (_, destination) = document.dereference(&destination).ok()?;
    let target = match destination.as_array().ok()?.first()? {
        Object::Reference(id) => *id,
        _ => return None,
    };
    pages
        .iter()
        .position(|page| *page == target)
        .map(|at| at as u32 + 1)
}

/// A PDF text string, in either of the two encodings.
fn text(document: &Document, object: &Object) -> Option<String> {
    let (_, object) = document.dereference(object).ok()?;
    let bytes = match object {
        Object::String(bytes, _) => bytes,
        _ => return None,
    };
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    Some(bytes.iter().map(|byte| *byte as char).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PDF whose objects are the ones given, with a cross-reference table
    /// that points at them. lopdf needs a real one, as any reader would.
    fn pdf(objects: &[(usize, &str)]) -> Vec<u8> {
        let mut out = b"%PDF-1.4\n".to_vec();
        let top = objects.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;
        let mut offsets = vec![0usize; top];
        for (number, body) in objects {
            offsets[*number] = out.len();
            out.extend_from_slice(format!("{number} 0 obj\n{body}\nendobj\n").as_bytes());
        }
        let start = out.len();
        out.extend_from_slice(format!("xref\n0 {top}\n0000000000 65535 f \n").as_bytes());
        for offset in offsets.iter().take(top).skip(1) {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {top} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n")
                .as_bytes(),
        );
        out
    }

    #[test]
    fn a_pdf_with_no_outline_says_so_rather_than_failing() {
        let minimal = pdf(&[
            (1, "<< /Type /Catalog /Pages 2 0 R >>"),
            (2, "<< /Type /Pages /Count 1 /Kids [3 0 R] >>"),
            (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
        ]);
        assert_eq!(read(&minimal), Err(Missing::None("outline")));
    }

    #[test]
    fn an_outline_comes_back_nested_and_with_its_pages() {
        let doc = pdf(&[
            (1, "<< /Type /Catalog /Pages 2 0 R /Outlines 10 0 R >>"),
            (2, "<< /Type /Pages /Count 2 /Kids [3 0 R 4 0 R] >>"),
            (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
            (4, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
            (10, "<< /Type /Outlines /First 11 0 R /Count 2 >>"),
            (
                11,
                "<< /Title (Part One) /Dest [3 0 R /Fit] /First 12 0 R /Next 13 0 R >>",
            ),
            (
                12,
                "<< /Title (I. The Setting) /Dest [3 0 R /Fit] /Parent 11 0 R >>",
            ),
            (
                13,
                "<< /Title (Part Two) /Dest [4 0 R /Fit] /Parent 10 0 R >>",
            ),
        ]);
        let entries = read(&doc).expect("an outline");
        assert_eq!(
            entries
                .iter()
                .map(|e| (e.depth, e.title.as_str(), e.page))
                .collect::<Vec<_>>(),
            [
                (0, "Part One", Some(1)),
                (1, "I. The Setting", Some(1)),
                (0, "Part Two", Some(2)),
            ]
        );
    }

    #[test]
    fn an_outline_that_points_at_itself_does_not_loop_forever() {
        let doc = pdf(&[
            (1, "<< /Type /Catalog /Pages 2 0 R /Outlines 10 0 R >>"),
            (2, "<< /Type /Pages /Count 1 /Kids [3 0 R] >>"),
            (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
            (10, "<< /Type /Outlines /First 11 0 R >>"),
            (11, "<< /Title (Round) /Next 12 0 R >>"),
            (12, "<< /Title (And round) /Next 11 0 R >>"),
        ]);
        let entries = read(&doc).expect("an outline");
        assert_eq!(entries.len(), 2, "a cycle was followed more than once");
    }

    #[test]
    fn what_is_not_a_pdf_says_why() {
        assert!(matches!(
            read(b"%PDF-1.4 truncated"),
            Err(Missing::Unreadable(_))
        ));
    }
}
