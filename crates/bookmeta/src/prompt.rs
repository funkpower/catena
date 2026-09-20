//! A question to put to a model, for the fields a file did not answer.
//!
//! Nothing here sends anything. The prompt is composed and printed, and what
//! happens to it is the caller's business — their model, their machine, their
//! GPU. That keeps this tool what every other tool here is: the same output
//! for the same input, on every run and every machine, with no network and
//! nothing to install.
//!
//! The prompt is part of the interface, not an implementation detail. It is
//! versioned with the records and printed where anyone can read it, because a
//! prompt that changes quietly is worse than one you cannot see at all.

use crate::Metadata;

/// The version of the prompt below.
///
/// Bumped when the wording changes in a way that could change an answer, so a
/// caller keeping results can tell which prompt produced them.
pub const PROMPT_VERSION: u32 = 1;

/// How much of a document to show a model.
///
/// The front matter is where a title page is. Past a few thousand characters
/// it is the body, which says nothing about who published it.
pub const EXCERPT_CHARS: usize = 3000;

/// Compose the question for what a file did not say about itself.
///
/// `excerpt` is the opening of the document, where there is one to hand;
/// without it the prompt asks about the metadata alone, which is worth less
/// but still worth something when the title is a scratch filename.
///
/// `None` when the file answered everything, because there is nothing to ask
/// and a call to a model would be waste.
pub fn compose(meta: &Metadata, excerpt: Option<&str>) -> Option<String> {
    let missing = meta.missing();
    if missing.is_empty() {
        return None;
    }

    let mut out = String::new();
    out.push_str(
        "You are cataloguing a book. Below is what its file says about \
         itself, and the opening of the text where one was available.\n\n",
    );
    out.push_str("Known:\n");
    out.push_str(&known(meta));
    out.push_str("\nMissing: ");
    out.push_str(&missing.join(", "));
    out.push_str("\n\n");

    if let Some(excerpt) = excerpt {
        let excerpt: String = excerpt.chars().take(EXCERPT_CHARS).collect();
        out.push_str("Opening of the text:\n---\n");
        out.push_str(excerpt.trim());
        out.push_str("\n---\n\n");
    }

    out.push_str(
        "Reply with one JSON object and nothing else. Use only these keys, \
         and only for fields listed as missing: title, authors, publisher, \
         date, language. `authors` is a list. `language` is a two-letter \
         code. Use null for anything the text does not settle — a guess is \
         worse than an absence, because nobody downstream can tell them \
         apart.\n",
    );
    Some(out)
}

/// The fields that are already known, as the prompt shows them.
fn known(meta: &Metadata) -> String {
    let mut out = String::new();
    let mut line = |name: &str, value: &str| {
        if !value.is_empty() {
            out.push_str(&format!("  {name}: {value}\n"));
        }
    };
    line("path", &meta.path);
    line("title", meta.title.as_deref().unwrap_or(""));
    line("authors", &meta.authors.join("; "));
    line("publisher", meta.publisher.as_deref().unwrap_or(""));
    line("date", meta.date.as_deref().unwrap_or(""));
    line("language", meta.language.as_deref().unwrap_or(""));
    for (scheme, value) in &meta.identifiers {
        line(scheme, value);
    }
    out
}

/// Take a model's reply and fill in the fields that were missing.
///
/// Only the missing ones: a model that volunteers a better title for a book
/// whose title was already known is overruled, because what the file says is
/// evidence and what a model says is a guess. A reply that is not one JSON
/// object is refused rather than half-applied.
pub fn merge(meta: &mut Metadata, reply: &str) -> Result<Vec<&'static str>, String> {
    let json = object_in(reply).ok_or("no JSON object in the reply")?;
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("not readable as JSON: {e}"))?;

    let missing = meta.missing();
    let mut filled = Vec::new();
    let text = |value: &serde_json::Value, key: &str| -> Option<String> {
        let found = value.get(key)?.as_str()?.trim().to_string();
        match found.is_empty() {
            true => None,
            false => Some(found),
        }
    };

    if missing.contains(&"title") {
        if let Some(title) = text(&value, "title") {
            meta.title = Some(title);
            filled.push("title");
        }
    }
    if missing.contains(&"authors") {
        let authors: Vec<String> = value
            .get("authors")
            .and_then(serde_json::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|name| name.as_str())
                    .map(|name| name.trim().to_string())
                    .filter(|name| !name.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        if !authors.is_empty() {
            meta.authors = authors;
            filled.push("authors");
        }
    }
    for (key, slot) in [
        ("publisher", &mut meta.publisher),
        ("date", &mut meta.date),
        ("language", &mut meta.language),
    ] {
        if missing.contains(&key) {
            if let Some(found) = text(&value, key) {
                *slot = Some(found);
                filled.push(match key {
                    "publisher" => "publisher",
                    "date" => "date",
                    _ => "language",
                });
            }
        }
    }

    if !filled.is_empty() {
        meta.note(format!(
            "{} filled in from a model, prompt version {PROMPT_VERSION}",
            filled.join(", ")
        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Source;

    fn bare() -> Metadata {
        let mut meta = Metadata::empty("scan.pdf", Source::PdfInfo);
        meta.title = Some("Microsoft Word - ch3.doc".into());
        meta
    }

    #[test]
    fn a_file_that_answered_everything_is_not_worth_asking_about() {
        let mut meta = bare();
        meta.authors = vec!["Palmer".into()];
        meta.publisher = Some("Faber".into());
        meta.date = Some("1984".into());
        meta.language = Some("en".into());
        assert_eq!(compose(&meta, None), None);
    }

    #[test]
    fn the_prompt_says_what_is_known_and_what_is_wanted() {
        let prompt = compose(&bare(), Some("THE PHILOKALIA\n\nVolume 3")).expect("a prompt");
        assert!(prompt.contains("Microsoft Word - ch3.doc"), "{prompt}");
        assert!(
            prompt.contains("Missing: authors, publisher, date, language"),
            "{prompt}"
        );
        assert!(prompt.contains("THE PHILOKALIA"), "{prompt}");
    }

    #[test]
    fn the_same_record_composes_the_same_prompt_every_time() {
        assert_eq!(compose(&bare(), Some("x")), compose(&bare(), Some("x")));
    }

    #[test]
    fn an_excerpt_is_cut_rather_than_sent_whole() {
        let huge = "a".repeat(EXCERPT_CHARS * 3);
        let prompt = compose(&bare(), Some(&huge)).expect("a prompt");
        assert!(prompt.len() < EXCERPT_CHARS * 2);
    }

    #[test]
    fn a_reply_fills_in_only_what_was_missing() {
        let mut meta = bare();
        let filled = merge(
            &mut meta,
            r#"{"title":"The Philokalia","authors":["Palmer","Ware"],"publisher":"Faber","date":"1984","language":"en"}"#,
        )
        .expect("a reply");
        // The title was present, however bad, so the model does not overrule it.
        assert_eq!(meta.title.as_deref(), Some("Microsoft Word - ch3.doc"));
        assert_eq!(meta.authors, ["Palmer", "Ware"]);
        assert_eq!(filled, ["authors", "publisher", "date", "language"]);
    }

    #[test]
    fn a_null_is_an_absence_and_not_a_value() {
        let mut meta = bare();
        let filled = merge(
            &mut meta,
            r#"{"authors":null,"publisher":"","date":"1984"}"#,
        )
        .expect("a reply");
        assert_eq!(filled, ["date"]);
        assert!(meta.authors.is_empty());
        assert_eq!(meta.publisher, None);
    }

    #[test]
    fn json_wrapped_in_prose_or_a_fence_is_still_found() {
        for reply in [
            "Here you go:\n```json\n{\"date\":\"1984\"}\n```\nHope that helps.",
            "{\"date\":\"1984\"}",
            "Sure! {\"date\":\"1984\"}",
        ] {
            let mut meta = bare();
            assert_eq!(
                merge(&mut meta, reply).expect("a reply"),
                ["date"],
                "{reply}"
            );
        }
    }

    #[test]
    fn a_brace_inside_a_string_does_not_end_the_object() {
        let mut meta = bare();
        merge(&mut meta, r#"{"publisher":"Faber } Faber","date":"1984"}"#).expect("a reply");
        assert_eq!(meta.publisher.as_deref(), Some("Faber } Faber"));
        assert_eq!(meta.date.as_deref(), Some("1984"));
    }

    #[test]
    fn a_reply_that_is_not_json_is_refused_rather_than_half_applied() {
        let mut meta = bare();
        assert!(merge(&mut meta, "I could not tell.").is_err());
        assert!(merge(&mut meta, "{not json}").is_err());
        assert!(meta.notes.is_empty());
    }
}
