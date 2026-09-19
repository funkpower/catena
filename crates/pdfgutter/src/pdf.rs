//! The page geometry, and the split itself.
//!
//! Splitting is a change to two rectangles per page. The content stream is
//! not touched, not re-rendered and not even read: both halves of a sheet
//! point at the same stream and differ only in how much of it they show.

use std::fmt;
use std::path::Path;

use lopdf::{Dictionary, Document, Object, ObjectId};

/// The four page attributes a page may inherit from the tree above it.
const INHERITED: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

/// The boxes that describe a region smaller than the page, which a half-page
/// would leave pointing at the wrong place.
const NARROWING: [&[u8]; 3] = [b"BleedBox", b"TrimBox", b"ArtBox"];

/// How far up a page tree to look for an inherited attribute before deciding
/// the file is looping.
const MAX_DEPTH: usize = 32;

/// The default page, for a file whose pages carry no box at all: US Letter,
/// which is what every reader falls back to.
const DEFAULT_BOX: [f64; 4] = [0.0, 0.0, 612.0, 792.0];

/// What can be wrong with a PDF.
#[derive(Debug)]
pub enum Error {
    /// It could not be read at all.
    Unreadable(String),
    /// It is encrypted, and no password was offered.
    Encrypted,
    /// It has no pages.
    Empty,
    /// The page tree does not add up: the file says one number of pages and
    /// walking it finds another.
    Miscounted {
        /// What the file says.
        declared: usize,
        /// What walking the tree found.
        found: usize,
    },
    /// It could not be written.
    Unwritable(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unreadable(why) => write!(f, "{why}"),
            Error::Encrypted => write!(f, "it is encrypted"),
            Error::Empty => write!(f, "it has no pages"),
            Error::Miscounted { declared, found } => write!(
                f,
                "it says it has {declared} pages and walking it finds {found}"
            ),
            Error::Unwritable(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

/// A page's shape as a reader sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    /// The media box, left, bottom, right, top, with the corners in order.
    pub media: [f64; 4],
    /// The crop box, where the page names one. This is what a reader shows
    /// and what a renderer draws, so it is what everything here measures.
    pub crop: Option<[f64; 4]>,
    /// A quarter turn clockwise per unit, as the file asks for it.
    pub rotate: i64,
}

impl Geometry {
    /// The rectangle a reader actually shows.
    ///
    /// A crop box outside the media box is meaningless, so it is clipped to
    /// it, and one that clips away the whole page is ignored.
    pub fn visible(self) -> [f64; 4] {
        let Some(crop) = self.crop else {
            return self.media;
        };
        let clipped = [
            crop[0].max(self.media[0]),
            crop[1].max(self.media[1]),
            crop[2].min(self.media[2]),
            crop[3].min(self.media[3]),
        ];
        match clipped[2] > clipped[0] && clipped[3] > clipped[1] {
            true => clipped,
            false => self.media,
        }
    }

    /// Width and height as displayed, which is the visible rectangle turned
    /// by whatever the page's rotation asks for.
    pub fn displayed(self) -> (f64, f64) {
        let visible = self.visible();
        let width = visible[2] - visible[0];
        let height = visible[3] - visible[1];
        match self.quarter_turns() % 2 {
            0 => (width, height),
            _ => (height, width),
        }
    }

    /// Width over height, as displayed.
    pub fn ratio(self) -> f64 {
        let (width, height) = self.displayed();
        crate::aspect_ratio(width, height)
    }

    /// The rotation as a number of quarter turns, from zero to three.
    ///
    /// A file may write any multiple of ninety, negative ones included.
    fn quarter_turns(self) -> i64 {
        (self.rotate / 90).rem_euclid(4)
    }

    /// The two halves of the sheet, in the order a reader would read them.
    ///
    /// Which way to cut depends on the rotation: a sheet turned on its side
    /// has its two pages one above the other in the file, however they look.
    pub fn halves(self) -> [[f64; 4]; 2] {
        let [left, bottom, right, top] = self.visible();
        let middle_x = (left + right) / 2.0;
        let middle_y = (bottom + top) / 2.0;

        let west = [left, bottom, middle_x, top];
        let east = [middle_x, bottom, right, top];
        let south = [left, bottom, right, middle_y];
        let north = [left, middle_y, right, top];

        match self.quarter_turns() {
            0 => [west, east],
            1 => [south, north],
            2 => [east, west],
            _ => [north, south],
        }
    }
}

/// A PDF, open and ready to be measured or cut.
pub struct Pdf {
    document: Document,
    pages: Vec<ObjectId>,
}

impl Pdf {
    /// Open a file.
    ///
    /// Refuses a document whose page tree does not add up. Walking a tree and
    /// finding fewer pages than the file says it has means there is a page
    /// here this does not understand, and the one thing that must not then
    /// happen is rewriting the tree around it — a page left out of the new
    /// tree is a page deleted.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let document = Document::load(path).map_err(|e| Error::Unreadable(e.to_string()))?;
        if document.is_encrypted() {
            return Err(Error::Encrypted);
        }
        let pages: Vec<ObjectId> = document.get_pages().into_values().collect();
        if pages.is_empty() {
            return Err(Error::Empty);
        }
        if let Some(declared) = declared_pages(&document) {
            if declared != pages.len() {
                return Err(Error::Miscounted {
                    declared,
                    found: pages.len(),
                });
            }
        }
        Ok(Pdf { document, pages })
    }

    /// How many pages it has.
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Whether it has no pages. It never does; `open` refuses one.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// The shape of one page, counting from zero.
    pub fn geometry(&self, index: usize) -> Geometry {
        let id = self.pages[index];
        let media = self
            .inherited(id, b"MediaBox")
            .and_then(|object| rectangle(&self.document, object))
            .unwrap_or(DEFAULT_BOX);
        let crop = self
            .inherited(id, b"CropBox")
            .and_then(|object| rectangle(&self.document, object));
        let rotate = self
            .inherited(id, b"Rotate")
            .and_then(|object| self.document.dereference(object).ok())
            .and_then(|(_, object)| object.as_i64().ok())
            .unwrap_or(0);
        Geometry {
            media,
            crop,
            rotate,
        }
    }

    /// An attribute of a page, from the page itself or from the tree above it.
    fn inherited(&self, page: ObjectId, key: &[u8]) -> Option<&Object> {
        let mut at = page;
        for _ in 0..MAX_DEPTH {
            let dictionary = self.document.get_dictionary(at).ok()?;
            if let Ok(object) = dictionary.get(key) {
                return Some(object);
            }
            at = dictionary.get(b"Parent").ok()?.as_reference().ok()?;
        }
        None
    }

    /// Cut every sheet wide enough to hold two pages into two, and leave the
    /// rest alone.
    ///
    /// Returns how many pages the document now has.
    /// Cut every sheet wide enough to hold two pages into two, and leave the
    /// rest alone.
    ///
    /// Returns how many pages the document now has. A document with nothing
    /// to cut is not touched at all, so writing it back out is a copy.
    pub fn split(&mut self, min_ratio: f64) -> usize {
        let wide: Vec<bool> = (0..self.pages.len())
            .map(|index| self.geometry(index).ratio() > min_ratio)
            .collect();
        if !wide.iter().any(|wide| *wide) {
            return self.pages.len();
        }

        // Whatever a page would have inherited has to be written onto it
        // first, because the tree it would have inherited from is about to be
        // replaced by a flat one.
        self.settle_inherited();

        let mut order: Vec<ObjectId> = Vec::with_capacity(self.pages.len());
        for (index, wide) in wide.iter().enumerate() {
            let id = self.pages[index];
            if !wide {
                order.push(id);
                continue;
            }

            // Both halves are new objects and the original is left as it was,
            // so a page the tree happens to list twice is cut twice rather
            // than cut once and then measured after the fact.
            let Ok(original) = self.document.get_dictionary(id).cloned() else {
                order.push(id);
                continue;
            };
            let geometry = self.geometry(index);
            for half in geometry.halves() {
                let mut page = original.clone();
                narrow(&mut page, half);
                keep_annotations_within(&self.document, &mut page, half);
                order.push(self.document.add_object(Object::Dictionary(page)));
            }
        }

        self.reparent(&order);
        self.pages = order;
        self.pages.len()
    }

    /// Write the result.
    pub fn save(&mut self, path: &Path) -> Result<(), Error> {
        self.document
            .save(path)
            .map(|_| ())
            .map_err(|e| Error::Unwritable(e.to_string()))
    }

    /// Copy every inheritable attribute down onto the pages that rely on it.
    fn settle_inherited(&mut self) {
        for page in self.pages.clone() {
            for key in INHERITED {
                let Some(value) = self.inherited(page, key).cloned() else {
                    continue;
                };
                if let Ok(dictionary) = self.document.get_dictionary_mut(page) {
                    if !dictionary.has(key) {
                        dictionary.set(key, value);
                    }
                }
            }
        }
    }

    /// Replace the page tree with one flat node holding the pages in order.
    fn reparent(&mut self, order: &[ObjectId]) {
        let Ok(root) = self
            .document
            .catalog()
            .and_then(|catalog| catalog.get(b"Pages"))
            .and_then(Object::as_reference)
        else {
            return;
        };

        for page in order {
            if let Ok(dictionary) = self.document.get_dictionary_mut(*page) {
                dictionary.set("Parent", Object::Reference(root));
            }
        }

        let kids: Vec<Object> = order.iter().map(|id| Object::Reference(*id)).collect();
        if let Ok(pages) = self.document.get_dictionary_mut(root) {
            pages.set("Kids", Object::Array(kids));
            pages.set("Count", Object::Integer(order.len() as i64));
        }
    }
}

/// Point a page dictionary at half the sheet.
fn narrow(page: &mut Dictionary, rectangle: [f64; 4]) {
    let value: Vec<Object> = rectangle
        .iter()
        .map(|edge| Object::Real(*edge as f32))
        .collect();
    page.set("MediaBox", Object::Array(value.clone()));
    page.set("CropBox", Object::Array(value));
    // These describe a region inside the old page, and half of them now lie
    // outside it entirely.
    for key in NARROWING {
        page.remove(key);
    }
}

/// Drop the annotations that belong to the other half.
///
/// A link sitting on the left page would otherwise be attached to the right
/// one as well, where a reader would show it over whatever text is now in
/// that position.
fn keep_annotations_within(document: &Document, page: &mut Dictionary, half: [f64; 4]) {
    let Ok(annotations) = page.get(b"Annots").and_then(|object| {
        document
            .dereference(object)
            .map(|(_, object)| object)
            .and_then(Object::as_array)
    }) else {
        return;
    };

    let kept: Vec<Object> = annotations
        .iter()
        .filter(|annotation| {
            document
                .dereference(annotation)
                .ok()
                .and_then(|(_, object)| object.as_dict().ok())
                .and_then(|dictionary| dictionary.get(b"Rect").ok())
                .and_then(|object| rectangle(document, object))
                .is_none_or(|rect| centre_within(rect, half))
        })
        .cloned()
        .collect();

    match kept.is_empty() {
        true => {
            page.remove(b"Annots");
        }
        false => page.set("Annots", Object::Array(kept)),
    }
}

/// Whether one rectangle's middle sits inside another.
///
/// The middle rather than any overlap, because an annotation straddling the
/// gutter has to land on one page and not both.
fn centre_within(rect: [f64; 4], half: [f64; 4]) -> bool {
    let (x, y) = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
    (half[0]..=half[2]).contains(&x) && (half[1]..=half[3]).contains(&y)
}

/// How many pages the file says it has.
fn declared_pages(document: &Document) -> Option<usize> {
    let root = document
        .catalog()
        .and_then(|catalog| catalog.get(b"Pages"))
        .and_then(Object::as_reference)
        .ok()?;
    let count = document.get_dictionary(root).ok()?.get(b"Count").ok()?;
    let (_, count) = document.dereference(count).ok()?;
    usize::try_from(count.as_i64().ok()?).ok()
}

/// Four numbers, in the order a PDF rectangle gives them, with the corners
/// put back in order.
///
/// A file may write a rectangle from any corner to any other, and readers are
/// required to cope, so a box that runs right to left is not a broken one.
fn rectangle(document: &Document, object: &Object) -> Option<[f64; 4]> {
    let (_, object) = document.dereference(object).ok()?;
    let array = object.as_array().ok()?;
    if array.len() != 4 {
        return None;
    }

    let mut edges = [0.0f64; 4];
    for (edge, object) in edges.iter_mut().zip(array) {
        let (_, object) = document.dereference(object).ok()?;
        *edge = f64::from(object.as_float().ok()?);
        if !edge.is_finite() {
            return None;
        }
    }
    Some([
        edges[0].min(edges[2]),
        edges[1].min(edges[3]),
        edges[0].max(edges[2]),
        edges[1].max(edges[3]),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(rotate: i64) -> Geometry {
        Geometry {
            media: [0.0, 0.0, 1000.0, 700.0],
            crop: None,
            rotate,
        }
    }

    fn cropped(crop: [f64; 4]) -> Geometry {
        Geometry {
            media: [0.0, 0.0, 1000.0, 700.0],
            crop: Some(crop),
            rotate: 0,
        }
    }

    #[test]
    fn the_visible_page_is_the_crop_box_where_there_is_one() {
        assert_eq!(
            cropped([200.0, 0.0, 1000.0, 700.0]).visible(),
            [200.0, 0.0, 1000.0, 700.0]
        );
        assert_eq!(sheet(0).visible(), [0.0, 0.0, 1000.0, 700.0]);
        // And the cut goes down the middle of what is shown, not of the sheet.
        assert_eq!(
            cropped([200.0, 0.0, 1000.0, 700.0]).halves()[0],
            [200.0, 0.0, 600.0, 700.0]
        );
    }

    #[test]
    fn a_crop_box_outside_the_sheet_is_clipped_to_it() {
        assert_eq!(
            cropped([-50.0, -50.0, 2000.0, 900.0]).visible(),
            [0.0, 0.0, 1000.0, 700.0]
        );
    }

    #[test]
    fn a_crop_box_that_leaves_nothing_is_ignored_rather_than_obeyed() {
        // A box outside the sheet entirely would otherwise give a page with
        // no area, which no reader shows and nothing can measure.
        assert_eq!(
            cropped([2000.0, 0.0, 3000.0, 700.0]).visible(),
            [0.0, 0.0, 1000.0, 700.0]
        );
        assert_eq!(
            cropped([10.0, 10.0, 10.0, 10.0]).visible(),
            [0.0, 0.0, 1000.0, 700.0]
        );
    }

    #[test]
    fn a_page_cropped_down_to_one_page_is_not_a_spread() {
        // The shape that matters: a wide sheet whose crop box shows only one
        // of the two pages on it. Measuring the sheet would split it again.
        let one_page = cropped([500.0, 0.0, 1000.0, 700.0]);
        assert_eq!(one_page.displayed(), (500.0, 700.0));
        assert!(one_page.ratio() < crate::MIN_RATIO);
    }

    #[test]
    fn an_upright_sheet_is_cut_left_then_right() {
        assert_eq!(
            sheet(0).halves(),
            [[0.0, 0.0, 500.0, 700.0], [500.0, 0.0, 1000.0, 700.0]]
        );
    }

    #[test]
    fn a_sheet_turned_upside_down_is_cut_right_then_left() {
        assert_eq!(
            sheet(180).halves(),
            [[500.0, 0.0, 1000.0, 700.0], [0.0, 0.0, 500.0, 700.0]]
        );
    }

    #[test]
    fn a_sheet_on_its_side_is_cut_across_rather_than_down() {
        // Turned a quarter clockwise, the file's bottom half is what the
        // reader sees on the left.
        assert_eq!(
            sheet(90).halves(),
            [[0.0, 0.0, 1000.0, 350.0], [0.0, 350.0, 1000.0, 700.0]]
        );
        assert_eq!(
            sheet(270).halves(),
            [[0.0, 350.0, 1000.0, 700.0], [0.0, 0.0, 1000.0, 350.0]]
        );
    }

    #[test]
    fn a_rotation_the_file_wrote_oddly_still_means_a_quarter_turn() {
        assert_eq!(sheet(-90).halves(), sheet(270).halves());
        assert_eq!(sheet(450).halves(), sheet(90).halves());
        assert_eq!(sheet(360).halves(), sheet(0).halves());
    }

    #[test]
    fn turning_a_sheet_on_its_side_turns_its_shape_with_it() {
        assert_eq!(sheet(0).displayed(), (1000.0, 700.0));
        assert_eq!(sheet(90).displayed(), (700.0, 1000.0));
        assert!(sheet(0).ratio() > crate::MIN_RATIO);
        assert!(sheet(90).ratio() < crate::MIN_RATIO);
    }

    #[test]
    fn the_two_halves_add_back_up_to_the_sheet() {
        for rotate in [0, 90, 180, 270] {
            let [first, second] = sheet(rotate).halves();
            let area = |b: [f64; 4]| (b[2] - b[0]) * (b[3] - b[1]);
            assert_eq!(area(first) + area(second), 1000.0 * 700.0, "{rotate}");
        }
    }
}
