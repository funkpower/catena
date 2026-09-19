//! The page geometry, and the split itself.
//!
//! Splitting is a change to two rectangles per page. The content stream is
//! not touched, not re-rendered and not even read: both halves of a sheet
//! point at the same stream and differ only in how much of it they show.

use std::fmt;
use std::path::Path;

use lopdf::{Document, Object, ObjectId};

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
    /// It could not be written.
    Unwritable(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unreadable(why) => write!(f, "{why}"),
            Error::Encrypted => write!(f, "it is encrypted"),
            Error::Empty => write!(f, "it has no pages"),
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
    /// A quarter turn clockwise per unit, as the file asks for it.
    pub rotate: i64,
}

impl Geometry {
    /// Width and height as displayed, which is the media box turned by
    /// whatever the page's rotation asks for.
    pub fn displayed(self) -> (f64, f64) {
        let width = self.media[2] - self.media[0];
        let height = self.media[3] - self.media[1];
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
        let [left, bottom, right, top] = self.media;
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
    pub fn open(path: &Path) -> Result<Self, Error> {
        let document = Document::load(path).map_err(|e| Error::Unreadable(e.to_string()))?;
        if document.is_encrypted() {
            return Err(Error::Encrypted);
        }
        let pages: Vec<ObjectId> = document.get_pages().into_values().collect();
        match pages.is_empty() {
            true => Err(Error::Empty),
            false => Ok(Pdf { document, pages }),
        }
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
        let rotate = self
            .inherited(id, b"Rotate")
            .and_then(|object| self.document.dereference(object).ok())
            .and_then(|(_, object)| object.as_i64().ok())
            .unwrap_or(0);
        Geometry { media, rotate }
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
    pub fn split(&mut self, min_ratio: f64) -> usize {
        // Whatever a page would have inherited has to be written onto it
        // first, because the tree it would have inherited from is about to be
        // replaced by a flat one.
        self.settle_inherited();

        let mut order: Vec<ObjectId> = Vec::with_capacity(self.pages.len());
        for index in 0..self.pages.len() {
            let id = self.pages[index];
            let geometry = self.geometry(index);
            if geometry.ratio() <= min_ratio {
                order.push(id);
                continue;
            }

            let [first, second] = geometry.halves();
            let twin = match self.document.get_dictionary(id) {
                Ok(dictionary) => dictionary.clone(),
                Err(_) => {
                    order.push(id);
                    continue;
                }
            };
            let twin = self.document.add_object(Object::Dictionary(twin));

            self.set_box(id, first);
            self.set_box(twin, second);
            order.push(id);
            order.push(twin);
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

    /// Point a page at half the sheet.
    fn set_box(&mut self, page: ObjectId, rectangle: [f64; 4]) {
        let Ok(dictionary) = self.document.get_dictionary_mut(page) else {
            return;
        };
        let value: Vec<Object> = rectangle
            .iter()
            .map(|edge| Object::Real(*edge as f32))
            .collect();
        dictionary.set("MediaBox", Object::Array(value.clone()));
        dictionary.set("CropBox", Object::Array(value));
        // These describe a region inside the old page, and half of them now
        // lie outside it entirely.
        for key in NARROWING {
            dictionary.remove(key);
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
            rotate,
        }
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
