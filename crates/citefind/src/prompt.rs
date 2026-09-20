//! A question to put to a model, for the references that would not parse.
//!
//! Nothing here sends anything. The prompt is composed and printed, and what
//! happens to it is the caller's business — their model, their machine, their
//! GPU. The tool stays what every tool here is: the same output for the same
//! input, on every machine, with no network and nothing to install.
//!
//! One prompt covers every unsettled entry rather than one prompt each. A
//! reference list is long and mostly ordinary; the entries worth asking about
//! are the mangled few, and they are cheaper to ask about together.

use crate::Reference;

/// The version of the prompt below.
///
/// Bumped when the wording changes in a way that could change an answer, so a
/// caller keeping results can tell which prompt produced them.
pub const PROMPT_VERSION: u32 = 1;

/// Compose one question covering every reference that did not fully parse.
///
/// `None` when every reference settled, because there is nothing to ask and
/// a call to a model would be waste.
pub fn compose(references: &[Reference]) -> Option<String> {
    let wanted: Vec<&Reference> = references
        .iter()
        .filter(|one| !one.missing().is_empty())
        .collect();
    if wanted.is_empty() {
        return None;
    }

    let mut out = String::new();
    out.push_str(
        "Below are entries from the bibliography or notes of a book. Each was \
         read as far as its punctuation allowed, and the fields that could not \
         be read off it are listed with it.\n\n",
    );
    for one in &wanted {
        out.push_str(&format!("--- {}\n", one.idx));
        out.push_str(&format!("text: {}\n", one.text));
        let known = known(one);
        if !known.is_empty() {
            out.push_str(&known);
        }
        out.push_str(&format!("wanted: {}\n\n", one.missing().join(", ")));
    }

    out.push_str(
        "Reply with one JSON object and nothing else. Its keys are the entry \
         numbers above, as strings; each value is an object using only these \
         keys, and only the ones listed as wanted for that entry: authors, \
         year, title, container. `authors` is a list of names as the entry \
         writes them. `year` is a four-digit number. `title` is the work \
         cited — for an article the article, not the journal. `container` is \
         the journal an article appeared in, or the publisher of a book.\n\n\
         Use null for anything the entry does not settle. A guess is worse \
         than an absence, because nobody downstream can tell them apart. Do \
         not resolve `ibid.` or `op. cit.` — an entry that only points at an \
         earlier one has nothing of its own to give.\n",
    );
    Some(out)
}

/// What was read off the entry, as the prompt shows it.
fn known(one: &Reference) -> String {
    let mut out = String::new();
    if !one.authors.is_empty() {
        out.push_str(&format!("authors: {}\n", one.authors.join("; ")));
    }
    if let Some(year) = one.year {
        out.push_str(&format!("year: {year}\n"));
    }
    if let Some(title) = &one.title {
        out.push_str(&format!("title: {title}\n"));
    }
    if let Some(container) = &one.container {
        out.push_str(&format!("container: {container}\n"));
    }
    out
}

/// Take a model's reply and fill in the fields that were missing.
///
/// Only the missing ones, and only on entries that asked: a model that
/// volunteers a better title for a reference whose title was read off the
/// page is overruled, because the page is evidence and the model is a guess.
/// A reply that is not one JSON object is refused rather than half-applied.
pub fn merge(references: &mut [Reference], reply: &str) -> Result<usize, String> {
    let json = object_in(reply).ok_or("no JSON object in the reply")?;
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("not readable as JSON: {e}"))?;
    let object = value.as_object().ok_or("the reply is not an object")?;

    let mut filled = 0;
    for one in references.iter_mut() {
        let missing = one.missing();
        if missing.is_empty() {
            continue;
        }
        let Some(answer) = object.get(&one.idx.to_string()) else {
            continue;
        };
        let mut touched = false;

        if missing.contains(&"authors") {
            let authors: Vec<String> = answer
                .get("authors")
                .and_then(serde_json::Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(|name| name.trim().to_string())
                        .filter(|name| !name.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            if !authors.is_empty() {
                one.authors = authors;
                touched = true;
            }
        }
        if missing.contains(&"year") {
            let year = answer
                .get("year")
                .and_then(|found| match found {
                    serde_json::Value::Number(number) => number.as_u64(),
                    serde_json::Value::String(text) => text.trim().parse().ok(),
                    _ => None,
                })
                .filter(|year| (1000..=2100).contains(year));
            if let Some(year) = year {
                one.year = Some(year as u32);
                touched = true;
            }
        }
        for (key, slot) in [("title", &mut one.title), ("container", &mut one.container)] {
            if !missing.contains(&key) {
                continue;
            }
            let found = answer
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty());
            if let Some(found) = found {
                *slot = Some(found.to_string());
                touched = true;
            }
        }

        if touched {
            filled += 1;
        }
        one.unsettled = one.missing().iter().map(|s| s.to_string()).collect();
    }
    Ok(filled)
}

/// The first balanced `{…}` in a reply.
///
/// Models wrap JSON in prose and in code fences however they were asked not
/// to, and refusing a good answer over a fence helps nobody.
fn object_in(reply: &str) -> Option<&str> {
    let start = reply.find('{')?;
    let bytes = reply.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for at in start..bytes.len() {
        let byte = bytes[at];
        if in_string {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&reply[start..=at]);
                }
            }
            _ => {}
        }
    }
    None
}
