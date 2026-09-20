//! A C ABI, so this can be embedded rather than spawned.
//!
//! # Shape
//!
//! Text goes in as NUL-terminated UTF-8 and records come back as a
//! NUL-terminated JSON array — the same document the command line writes,
//! under the same stability promise. Nothing here is an opaque handle with
//! accessors to keep in step: the records already have a documented shape,
//! and every language with an FFI has a JSON parser.
//!
//! Every string this returns is owned by the caller and must be released with
//! [`catena_string_free`]. Passing it anything else is undefined.
//!
//! # Errors
//!
//! A function that can fail returns NULL and leaves a message in
//! [`catena_last_error`], which is thread-local and valid until the next
//! failing call on the same thread.
//!
//! # Threads
//!
//! Every function is safe to call from any thread. Nothing here holds state
//! between calls but the error message, which is per-thread.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_double, c_int, c_uint};
use std::ptr;

use crate::{ChunkBudgets, CueBudgets};

/// Bumped when the meaning or signature of anything here changes. Additions
/// leave it alone.
///
/// 2: `catena_segment_cues` gained an embeddings argument.
const ABI_VERSION: c_uint = 2;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_error(message: impl Into<Vec<u8>>) {
    let message = CString::new(message).unwrap_or_else(|_| c"error".into());
    LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(message));
}

fn clear_error() {
    LAST_ERROR.with(|slot| *slot.borrow_mut() = None);
}

/// The version of this ABI.
#[unsafe(no_mangle)]
pub extern "C" fn catena_abi_version() -> c_uint {
    ABI_VERSION
}

/// Why the last call on this thread failed, or NULL if none did.
///
/// # Safety
///
/// The pointer is valid until the next failing call on the same thread. Copy
/// it if you need it longer.
#[unsafe(no_mangle)]
pub extern "C" fn catena_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| match slot.borrow().as_ref() {
        Some(message) => message.as_ptr(),
        None => ptr::null(),
    })
}

/// Release a string this library returned.
///
/// # Safety
///
/// `text` must be a pointer this library returned and not yet freed, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_string_free(text: *mut c_char) {
    if !text.is_null() {
        drop(unsafe { CString::from_raw(text) });
    }
}

/// Borrow a caller's string as `&str`, or fail.
///
/// # Safety
///
/// `text` must be NUL-terminated and valid for reads, or NULL.
unsafe fn borrow<'a>(text: *const c_char, what: &str) -> Option<&'a str> {
    if text.is_null() {
        set_error(format!("{what} is NULL"));
        return None;
    }
    match unsafe { CStr::from_ptr(text) }.to_str() {
        Ok(text) => Some(text),
        Err(_) => {
            set_error(format!("{what} is not UTF-8"));
            None
        }
    }
}

/// Hand a string to the caller, or NULL if it cannot be made into one.
fn give(text: String) -> *mut c_char {
    match CString::new(text) {
        Ok(text) => text.into_raw(),
        Err(_) => {
            // Only reachable if a record held a NUL, which the readers refuse.
            set_error("the answer contains a NUL byte");
            ptr::null_mut()
        }
    }
}

/// Serialise records to JSON and hand them over.
fn give_json<T: serde::Serialize>(records: &T) -> *mut c_char {
    match serde_json::to_string(records) {
        Ok(json) => give(json),
        Err(e) => {
            set_error(format!("the answer does not serialise: {e}"));
            ptr::null_mut()
        }
    }
}

/// Read a JSON argument, or fail.
fn take_json<T: serde::de::DeserializeOwned>(json: &str, what: &str) -> Option<T> {
    match serde_json::from_str(json) {
        Ok(value) => Some(value),
        Err(e) => {
            set_error(format!("{what} is not the expected shape: {e}"));
            None
        }
    }
}

/// Cut markdown into retrievable passages. Returns a JSON array of chunks.
///
/// A budget of zero means the default for that budget, so a caller that only
/// cares about one of them can pass zero for the rest.
///
/// # Safety
///
/// `markdown` must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_chunk_markdown(
    markdown: *const c_char,
    target: usize,
    overlap: usize,
    min_tokens: usize,
    hard_max: usize,
) -> *mut c_char {
    clear_error();
    let Some(markdown) = (unsafe { borrow(markdown, "markdown") }) else {
        return ptr::null_mut();
    };
    let default = ChunkBudgets::default();
    let budgets = ChunkBudgets {
        target: or_default(target, default.target),
        overlap: match overlap {
            // Zero overlap is a real choice, unlike a zero budget.
            _ if target == 0 && overlap == 0 => default.overlap,
            overlap => overlap,
        },
        min_tokens: or_default(min_tokens, default.min_tokens),
        hard_max: or_default(hard_max, default.hard_max),
    };
    give_json(&crate::chunk_markdown(markdown, budgets))
}

fn or_default(value: usize, default: usize) -> usize {
    match value {
        0 => default,
        value => value,
    }
}

/// Clean converted markdown. Returns the cleaned text.
///
/// `pages` non-zero normalises page markers as well, which is not idempotent.
///
/// # Safety
///
/// `markdown` must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_scrub(markdown: *const c_char, pages: c_int) -> *mut c_char {
    clear_error();
    let Some(markdown) = (unsafe { borrow(markdown, "markdown") }) else {
        return ptr::null_mut();
    };
    give(match pages {
        0 => crate::scrub_markdown(markdown),
        _ => crate::scrub_markdown_pages(markdown),
    })
}

/// How many tokens a string is. Returns zero for NULL or invalid UTF-8, and
/// leaves a message in [`catena_last_error`].
///
/// # Safety
///
/// `text` must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_count_tokens(text: *const c_char) -> usize {
    clear_error();
    match unsafe { borrow(text, "text") } {
        Some(text) => crate::count_tokens(text),
        None => 0,
    }
}

/// Read any of the subtitle formats. Returns a JSON array of cues.
///
/// # Safety
///
/// `content` must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_parse_subtitles(
    content: *const c_char,
    keep_repeats: c_int,
) -> *mut c_char {
    clear_error();
    let Some(content) = (unsafe { borrow(content, "content") }) else {
        return ptr::null_mut();
    };
    let cues = crate::parse_subtitles(content);
    let cues = match keep_repeats {
        0 => crate::merge_repeats(&cues),
        _ => cues,
    };
    give_json(&cues)
}

/// Pack cues into passages. `cues` is a JSON array of cues; returns a JSON
/// array of chunks.
///
/// Zero for either budget means the default.
///
/// # Safety
///
/// `cues` must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_pack_cues(
    cues: *const c_char,
    seconds: c_double,
    max_tokens: usize,
) -> *mut c_char {
    clear_error();
    let Some(cues) = (unsafe { borrow(cues, "cues") }) else {
        return ptr::null_mut();
    };
    let Some(cues) = take_json::<Vec<cuekit::Cue>>(cues, "cues") else {
        return ptr::null_mut();
    };
    let default = CueBudgets::default();
    let budgets = CueBudgets {
        seconds: match seconds > 0.0 {
            true => seconds,
            false => default.seconds,
        },
        max_tokens: or_default(max_tokens, default.max_tokens),
    };
    give_json(&crate::pack_cues(&cues, budgets))
}

/// Cut a timed transcript into topic chapters. `cues` is a JSON array of
/// cues; returns a JSON array of segments.
///
/// A duration of zero or less is taken from the last cue.
///
/// # Safety
///
/// `cues` must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_segment_cues(
    cues: *const c_char,
    duration: c_double,
    embeddings: *const c_char,
) -> *mut c_char {
    clear_error();
    let Some(cues) = (unsafe { borrow(cues, "cues") }) else {
        return ptr::null_mut();
    };
    let Some(cues) = take_json::<Vec<cuekit::Cue>>(cues, "cues") else {
        return ptr::null_mut();
    };

    let vectors: Option<Vec<Vec<f64>>> = match embeddings.is_null() {
        true => None,
        false => {
            let Some(text) = (unsafe { borrow(embeddings, "embeddings") }) else {
                return ptr::null_mut();
            };
            match take_json::<Vec<Vec<f64>>>(text, "embeddings") {
                Some(vectors) => Some(vectors),
                None => return ptr::null_mut(),
            }
        }
    };
    give_json(&crate::segment_cues_with(
        &cues,
        duration,
        vectors.as_deref(),
    ))
}

/// The text of each window a transcript is measured in, to embed. Returns a
/// JSON array of strings.
///
/// # Safety
///
/// `cues` must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_cue_windows(cues: *const c_char) -> *mut c_char {
    clear_error();
    let Some(cues) = (unsafe { borrow(cues, "cues") }) else {
        return ptr::null_mut();
    };
    let Some(cues) = take_json::<Vec<cuekit::Cue>>(cues, "cues") else {
        return ptr::null_mut();
    };
    give_json(&crate::cue_windows(&cues))
}

/// Split a collection into the works inside it. Returns a JSON array of
/// sections, or NULL when the document is one work rather than a collection —
/// which is an answer, not a failure, and the reason is in
/// [`catena_last_error`].
///
/// # Safety
///
/// `markdown` must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_split_collection(markdown: *const c_char) -> *mut c_char {
    clear_error();
    let Some(markdown) = (unsafe { borrow(markdown, "markdown") }) else {
        return ptr::null_mut();
    };
    match crate::split_collection(markdown) {
        Ok(sections) => give_json(&sections),
        Err(why) => {
            set_error(why);
            ptr::null_mut()
        }
    }
}

/// Fuse ranked lists into one. `hits` is a JSON array of hits and `weights` a
/// JSON object of channel to weight, or NULL for none. Returns a JSON array.
///
/// Zero for `k` means the default; zero for `limit` means all of them.
///
/// # Safety
///
/// `hits` must be NUL-terminated UTF-8; `weights` likewise, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_fuse_hits(
    hits: *const c_char,
    k: c_double,
    weights: *const c_char,
    limit: usize,
) -> *mut c_char {
    clear_error();
    let Some(hits) = (unsafe { borrow(hits, "hits") }) else {
        return ptr::null_mut();
    };
    let Some(hits) = take_json::<Vec<corpus_core::Hit>>(hits, "hits") else {
        return ptr::null_mut();
    };

    let weights: Vec<(String, f64)> = if weights.is_null() {
        Vec::new()
    } else {
        let Some(text) = (unsafe { borrow(weights, "weights") }) else {
            return ptr::null_mut();
        };
        let Some(map) = take_json::<std::collections::HashMap<String, f64>>(text, "weights") else {
            return ptr::null_mut();
        };
        map.into_iter().collect()
    };

    let k = match k > 0.0 {
        true => k,
        false => rrf::K,
    };
    let limit = match limit {
        0 => None,
        limit => Some(limit),
    };
    give_json(&crate::fuse_hits(&hits, k, &weights, limit))
}

/// Say what a file is from its first bytes and its name. Returns a JSON doc
/// record.
///
/// # Safety
///
/// `name` must be NUL-terminated UTF-8, and `data` must be valid for `len`
/// bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_identify(
    name: *const c_char,
    data: *const u8,
    len: usize,
) -> *mut c_char {
    clear_error();
    let Some(name) = (unsafe { borrow(name, "name") }) else {
        return ptr::null_mut();
    };
    if data.is_null() && len != 0 {
        set_error("data is NULL");
        return ptr::null_mut();
    }
    let bytes: &[u8] = match len {
        0 => &[],
        len => unsafe { std::slice::from_raw_parts(data, len) },
    };
    give_json(&crate::identify(name, bytes))
}

/// The series a title belongs to. Returns a JSON proposal, or NULL when the
/// title says nothing about a series — which is most titles, and not an
/// error: `catena_last_error` is NULL in that case.
///
/// # Safety
///
/// `title` must be NUL-terminated UTF-8; `author` likewise, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_series_of(
    title: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_error();
    let Some(title) = (unsafe { borrow(title, "title") }) else {
        return ptr::null_mut();
    };
    let author = match author.is_null() {
        true => None,
        false => match unsafe { borrow(author, "author") } {
            Some(author) => Some(author),
            None => return ptr::null_mut(),
        },
    };
    match crate::series_of(title, author) {
        Some(found) => give_json(&found),
        None => ptr::null_mut(),
    }
}

/// Read an identifier out of whatever it arrived wrapped in. Returns a JSON
/// record, or NULL when the string is not one — an answer, not an error.
///
/// # Safety
///
/// `text` must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn catena_parse_identifier(text: *const c_char) -> *mut c_char {
    clear_error();
    let Some(text) = (unsafe { borrow(text, "text") }) else {
        return ptr::null_mut();
    };
    match crate::parse_identifier(text) {
        Some(found) => give_json(&found),
        None => ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Call through the ABI the way a C caller would, and take the answer
    /// back as a Rust string.
    fn through(call: impl FnOnce() -> *mut c_char) -> Option<String> {
        let raw = call();
        if raw.is_null() {
            return None;
        }
        let text = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        unsafe { catena_string_free(raw) };
        Some(text)
    }

    fn error() -> Option<String> {
        let raw = catena_last_error();
        match raw.is_null() {
            true => None,
            false => Some(
                unsafe { CStr::from_ptr(raw) }
                    .to_string_lossy()
                    .into_owned(),
            ),
        }
    }

    #[test]
    fn chunking_comes_back_as_the_records_the_command_writes() {
        let markdown = c"# One\n\n<!-- Page 41 -->\n\nThe obligation ran both ways, and contemporaries said so plainly whenever they were asked at all.\n";
        let json = through(|| unsafe { catena_chunk_markdown(markdown.as_ptr(), 0, 0, 10, 0) })
            .expect("a result");
        let chunks: Vec<serde_json::Value> = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(chunks[0]["page"], 41);
        assert_eq!(chunks[0]["chapter"], "One");
    }

    #[test]
    fn a_zero_budget_is_the_default_budget() {
        let markdown = c"# One\n\nbody\n";
        assert!(
            through(|| unsafe { catena_chunk_markdown(markdown.as_ptr(), 0, 0, 0, 0) }).is_some()
        );
    }

    #[test]
    fn a_null_argument_is_an_error_rather_than_a_crash() {
        assert!(through(|| unsafe { catena_chunk_markdown(ptr::null(), 0, 0, 0, 0) }).is_none());
        assert_eq!(error().as_deref(), Some("markdown is NULL"));
        assert_eq!(unsafe { catena_count_tokens(ptr::null()) }, 0);
        assert_eq!(error().as_deref(), Some("text is NULL"));
    }

    #[test]
    fn freeing_nothing_is_allowed() {
        unsafe { catena_string_free(ptr::null_mut()) };
    }

    #[test]
    fn a_document_that_is_one_work_says_why_rather_than_returning_nothing() {
        let markdown = c"# Only\n\nbody\n";
        assert!(through(|| unsafe { catena_split_collection(markdown.as_ptr()) }).is_none());
        assert!(error().expect("a reason").contains("sections"));
    }

    #[test]
    fn a_bad_json_argument_says_which_one() {
        let hits = c"not json";
        assert!(
            through(|| unsafe { catena_fuse_hits(hits.as_ptr(), 0.0, ptr::null(), 0) }).is_none()
        );
        assert!(
            error()
                .expect("a reason")
                .starts_with("hits is not the expected shape")
        );
    }

    #[test]
    fn identifying_nothing_is_identifying_an_empty_file() {
        let name = c"book.pdf";
        let json = through(|| unsafe { catena_identify(name.as_ptr(), ptr::null(), 0) })
            .expect("a result");
        assert!(json.contains("\"path\":\"book.pdf\""));
    }

    #[test]
    fn subtitles_round_trip_through_the_boundary() {
        let srt = c"1\n00:00:01,000 --> 00:00:04,000\nthe mill stood at the northern end\n\n";
        let cues = through(|| unsafe { catena_parse_subtitles(srt.as_ptr(), 0) }).expect("cues");
        let packed = CString::new(cues).expect("no NUL");
        let chunks =
            through(|| unsafe { catena_pack_cues(packed.as_ptr(), 0.0, 0) }).expect("chunks");
        assert!(chunks.contains("the mill stood at the northern end"));
    }

    #[test]
    fn a_title_with_no_series_is_null_and_not_an_error() {
        let title = c"Being and Time";
        assert!(through(|| unsafe { catena_series_of(title.as_ptr(), ptr::null()) }).is_none());
        assert_eq!(error(), None, "no series is an answer, not a failure");

        let series = c"The Philokalia Vol 3";
        let json = through(|| unsafe { catena_series_of(series.as_ptr(), ptr::null()) })
            .expect("a proposal");
        assert!(json.contains("\"volume\":3"));
    }

    #[test]
    fn an_identifier_is_normalised_across_the_boundary() {
        let text = c"https://doi.org/10.1000/182";
        let json = through(|| unsafe { catena_parse_identifier(text.as_ptr()) }).expect("a record");
        assert!(json.contains("\"id\":\"10.1000/182\""), "{json}");

        let nope = c"hello";
        assert!(through(|| unsafe { catena_parse_identifier(nope.as_ptr()) }).is_none());
        assert_eq!(error(), None, "not an identifier is an answer");
    }

    #[test]
    fn the_abi_version_is_the_one_the_header_declares() {
        assert_eq!(catena_abi_version(), 2);
    }
}
