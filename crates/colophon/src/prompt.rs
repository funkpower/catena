//! A question to put to a model, for what the pages did not settle.
//!
//! Nothing here sends anything. The prompt is composed and printed, and what
//! happens to it is the caller's business — their model, their machine,
//! their GPU. The tool stays what every tool here is: the same output for
//! the same input, on every machine, with no network and nothing installed.
//!
//! What the model is shown is not the front matter as it arrived. The pages
//! that advertise other books are taken out first, because handing them to a
//! model is how a book ends up catalogued as whichever of two hundred other
//! titles the model's eye fell on. It is shown the pages that describe this
//! book, told what was already read off them, and asked only for the rest.

use crate::{Printed, Source, page};

/// The version of the prompt below.
///
/// Bumped when the wording changes in a way that could change an answer, so
/// a caller keeping results can tell which prompt produced them.
pub const PROMPT_VERSION: u32 = 1;

/// How much of the front matter to show. Enough for a title page, a
/// copyright page and a contents; not enough to wander into the text.
const SHOWN: usize = 6000;

/// The fields a model may be asked for.
const ASKABLE: &[&str] = &["title", "authors", "publisher", "year", "subjects"];

fn known(record: &Printed) -> String {
    let mut out = String::new();
    if let Some(title) = &record.title {
        out.push_str(&format!("title already read: {title}\n"));
    }
    if !record.authors.is_empty() {
        out.push_str(&format!(
            "authors already read: {}\n",
            record.authors.join("; ")
        ));
    }
    if !record.isbns.is_empty() {
        out.push_str(&format!(
            "isbn printed in the book: {}\n",
            record.isbns.join(", ")
        ));
    }
    if let Some(publisher) = &record.publisher {
        out.push_str(&format!("publisher already read: {publisher}\n"));
    }
    out
}

/// The front matter with the advertising taken out.
pub fn readable_pages(text: &str, budget: usize) -> String {
    let mut out = String::new();
    for page in page::split(text, budget) {
        if matches!(page.kind, page::Kind::SeriesList | page::Kind::Blank) || page.lines.is_empty()
        {
            continue;
        }
        out.push_str(&format!("--- page {}\n", page.number));
        for line in &page.lines {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
        if out.len() >= SHOWN {
            break;
        }
    }
    out.truncate(SHOWN);
    out
}

/// Compose the question, or decline to.
///
/// `None` when the pages settled everything a model could help with, because
/// there is nothing to ask and a call to a model would be waste.
pub fn compose(record: &Printed, text: &str, budget: usize) -> Option<String> {
    let wanted: Vec<&str> = ASKABLE
        .iter()
        .copied()
        .filter(|field| record.unsettled.iter().any(|f| f == field))
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let pages = readable_pages(text, budget);
    if pages.trim().is_empty() {
        return None;
    }

    let mut out = String::from(
        "Below are the opening pages of one book, as text. Pages advertising \
         other books have already been removed.\n\n",
    );
    out.push_str(&pages);
    out.push('\n');
    let already = known(record);
    if !already.is_empty() {
        out.push_str("Read off these pages already:\n");
        out.push_str(&already);
        out.push('\n');
    }
    out.push_str(&format!("wanted: {}\n\n", wanted.join(", ")));
    out.push_str(
        "Reply with one JSON object and nothing else, using only the keys \
         listed as wanted. `title` is the book's own title, without its \
         subtitle. `authors` is a list of the people who wrote it, as the \
         book writes their names. `publisher` is the press. `year` is a \
         four-digit number. `subjects` is a list of at most five subject \
         headings for what the book is about.\n\n\
         Answer only from the pages above. Use null for anything they do not \
         settle: a guess is worse than an absence, because nobody downstream \
         can tell them apart. A title listed beside other titles is another \
         book's, not this one's.\n",
    );
    Some(out)
}

/// Fill in what the pages did not settle from a model's reply.
///
/// Only the fields that were unsettled are filled, and each one is marked as
/// having come from a model — a record that cannot say which of its fields
/// were read and which were answered is a record nobody can apply a policy
/// to. Returns how many fields were filled.
pub fn merge(record: &mut Printed, reply: &str) -> Result<usize, String> {
    let start = reply.find('{').ok_or("the reply holds no JSON object")?;
    let end = reply.rfind('}').ok_or("the reply holds no JSON object")?;
    let parsed: serde_json::Value = serde_json::from_str(&reply[start..=end])
        .map_err(|e| format!("the reply is not JSON: {e}"))?;
    let object = parsed.as_object().ok_or("the reply is not a JSON object")?;

    let mut filled = 0;
    let wanted = |record: &Printed, field: &str| record.unsettled.iter().any(|f| f == field);

    if wanted(record, "title") {
        if let Some(title) = object.get("title").and_then(|v| v.as_str()) {
            let title = title.trim();
            if !title.is_empty() {
                record.title = Some(title.to_string());
                record.read_from.insert("title".into(), Source::Model);
                filled += 1;
            }
        }
    }
    if wanted(record, "publisher") {
        if let Some(publisher) = object.get("publisher").and_then(|v| v.as_str()) {
            let publisher = publisher.trim();
            if !publisher.is_empty() {
                record.publisher = Some(publisher.to_string());
                record.read_from.insert("publisher".into(), Source::Model);
                filled += 1;
            }
        }
    }
    if wanted(record, "year") {
        if let Some(year) = object.get("year").and_then(serde_json::Value::as_u64) {
            if (1400..=2200).contains(&year) {
                record.year = Some(year as u16);
                record.read_from.insert("year".into(), Source::Model);
                filled += 1;
            }
        }
    }
    for (field, into) in [("authors", true), ("subjects", false)] {
        if !wanted(record, field) {
            continue;
        }
        let Some(list) = object.get(field).and_then(|v| v.as_array()) else {
            continue;
        };
        let values: Vec<String> = list
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if values.is_empty() {
            continue;
        }
        if into {
            record.authors = values;
        } else {
            record.subjects = values;
        }
        record.read_from.insert(field.into(), Source::Model);
        filled += 1;
    }

    record
        .unsettled
        .retain(|field| !record.read_from.contains_key(field));
    Ok(filled)
}
