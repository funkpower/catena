//! Python bindings.
//!
//! These hand back the record shapes as dictionaries — the same JSON every
//! tool writes to stdout, spelled the way Python spells things. A stage that
//! was reading NDJSON from a subprocess can call the function instead and get
//! the dictionaries it was already parsing.
//!
//! Built only under the `python` feature, which maturin turns on. See
//! Cargo.toml for why it cannot be on by default.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use crate::{ChunkBudgets, CueBudgets};

/// A record to a Python dictionary, by way of its JSON.
///
/// Through JSON rather than field by field so that the two forms cannot drift
/// apart: if a record gains a field, it appears here without anybody
/// remembering to add it.
fn to_py<'py, T: serde::Serialize>(py: Python<'py>, value: &T) -> PyResult<Bound<'py, PyAny>> {
    let json = serde_json::to_value(value)
        .map_err(|e| PyValueError::new_err(format!("record does not serialise: {e}")))?;
    json_to_py(py, &json)
}

/// Every record in a list, likewise.
fn list_to_py<'py, T: serde::Serialize>(
    py: Python<'py>,
    values: &[T],
) -> PyResult<Bound<'py, PyList>> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        out.push(to_py(py, value)?);
    }
    PyList::new(py, out)
}

fn json_to_py<'py>(py: Python<'py>, value: &serde_json::Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match value {
        serde_json::Value::Null => py.None().into_bound(py),
        serde_json::Value::Bool(b) => b.into_pyobject(py)?.to_owned().into_any(),
        serde_json::Value::Number(n) => match (n.as_u64(), n.as_i64(), n.as_f64()) {
            (Some(u), _, _) => u.into_pyobject(py)?.into_any(),
            (_, Some(i), _) => i.into_pyobject(py)?.into_any(),
            (_, _, Some(f)) => f.into_pyobject(py)?.into_any(),
            // JSON cannot hold a number this is not one of, so this is
            // unreachable rather than a case worth inventing a value for.
            _ => return Err(PyValueError::new_err(format!("{n} is not a number"))),
        },
        serde_json::Value::String(s) => s.into_pyobject(py)?.into_any(),
        serde_json::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(json_to_py(py, item)?);
            }
            PyList::new(py, out)?.into_any()
        }
        serde_json::Value::Object(fields) => {
            let dict = PyDict::new(py);
            for (key, value) in fields {
                dict.set_item(key, json_to_py(py, value)?)?;
            }
            dict.into_any()
        }
    })
}

/// A Python dictionary back to a record, the same way round.
fn from_py<T: serde::de::DeserializeOwned>(value: &Bound<'_, PyAny>) -> PyResult<T> {
    let json: String = value
        .py()
        .import("json")?
        .call_method1("dumps", (value,))?
        .extract()?;
    serde_json::from_str(&json)
        .map_err(|e| PyValueError::new_err(format!("not a record of the expected shape: {e}")))
}

/// Every dictionary in a sequence.
fn from_py_list<T: serde::de::DeserializeOwned>(values: &Bound<'_, PyAny>) -> PyResult<Vec<T>> {
    let mut out = Vec::new();
    for value in values.try_iter()? {
        out.push(from_py(&value?)?);
    }
    Ok(out)
}

/// Cut markdown into retrievable passages, one dictionary each.
#[pyfunction]
#[pyo3(signature = (markdown, *, target = None, overlap = None, min_tokens = None, hard_max = None))]
fn chunk_markdown<'py>(
    py: Python<'py>,
    markdown: &str,
    target: Option<usize>,
    overlap: Option<usize>,
    min_tokens: Option<usize>,
    hard_max: Option<usize>,
) -> PyResult<Bound<'py, PyList>> {
    let default = ChunkBudgets::default();
    let budgets = ChunkBudgets {
        target: target.unwrap_or(default.target),
        overlap: overlap.unwrap_or(default.overlap),
        min_tokens: min_tokens.unwrap_or(default.min_tokens),
        hard_max: hard_max.unwrap_or(default.hard_max),
    };
    list_to_py(py, &crate::chunk_markdown(markdown, budgets))
}

/// Clean converted markdown.
#[pyfunction]
#[pyo3(signature = (markdown, *, pages = false))]
fn scrub(markdown: &str, pages: bool) -> String {
    match pages {
        true => crate::scrub_markdown_pages(markdown),
        false => crate::scrub_markdown(markdown),
    }
}

/// How many tokens a string is.
#[pyfunction]
fn count_tokens(text: &str) -> usize {
    crate::count_tokens(text)
}

/// Read any of the subtitle formats into cues.
#[pyfunction]
#[pyo3(signature = (content, *, keep_repeats = false))]
fn parse_subtitles<'py>(
    py: Python<'py>,
    content: &str,
    keep_repeats: bool,
) -> PyResult<Bound<'py, PyList>> {
    let cues = crate::parse_subtitles(content);
    let cues = match keep_repeats {
        true => cues,
        false => crate::merge_repeats(&cues),
    };
    list_to_py(py, &cues)
}

/// Pack cues into passages worth embedding.
#[pyfunction]
#[pyo3(signature = (cues, *, seconds = None, max_tokens = None))]
fn pack_cues<'py>(
    py: Python<'py>,
    cues: &Bound<'py, PyAny>,
    seconds: Option<f64>,
    max_tokens: Option<usize>,
) -> PyResult<Bound<'py, PyList>> {
    let default = CueBudgets::default();
    let budgets = CueBudgets {
        seconds: seconds.unwrap_or(default.seconds),
        max_tokens: max_tokens.unwrap_or(default.max_tokens),
    };
    let cues: Vec<cuekit::Cue> = from_py_list(cues)?;
    list_to_py(py, &crate::pack_cues(&cues, budgets))
}

/// Cut a timed transcript into topic chapters.
///
/// `duration` is the recording's length in seconds; omit it and the last cue
/// decides.
/// `embeddings` is one vector per window, in the order `cue_windows` gives
/// them; without it the windows are compared by the words they repeat.
#[pyfunction]
#[pyo3(signature = (cues, *, duration = None, embeddings = None))]
fn segment_cues<'py>(
    py: Python<'py>,
    cues: &Bound<'py, PyAny>,
    duration: Option<f64>,
    embeddings: Option<Vec<Vec<f64>>>,
) -> PyResult<Bound<'py, PyList>> {
    let cues: Vec<cuekit::Cue> = from_py_list(cues)?;
    list_to_py(
        py,
        &crate::segment_cues_with(&cues, duration.unwrap_or(0.0), embeddings.as_deref()),
    )
}

/// The text of each window a transcript is measured in, to embed.
#[pyfunction]
fn cue_windows(cues: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let cues: Vec<cuekit::Cue> = from_py_list(cues)?;
    Ok(crate::cue_windows(&cues))
}

/// Split a collection into the works inside it.
///
/// Raises `ValueError` when the document is one work rather than a
/// collection, with the reason each candidate was rejected.
#[pyfunction]
fn split_collection<'py>(py: Python<'py>, markdown: &str) -> PyResult<Bound<'py, PyList>> {
    match crate::split_collection(markdown) {
        Ok(sections) => list_to_py(py, &sections),
        Err(why) => Err(PyValueError::new_err(why)),
    }
}

/// Fuse ranked lists into one.
#[pyfunction]
#[pyo3(signature = (hits, *, k = None, weights = None, limit = None))]
fn fuse_hits<'py>(
    py: Python<'py>,
    hits: &Bound<'py, PyAny>,
    k: Option<f64>,
    weights: Option<std::collections::HashMap<String, f64>>,
    limit: Option<usize>,
) -> PyResult<Bound<'py, PyList>> {
    let hits: Vec<corpus_core::Hit> = from_py_list(hits)?;
    let weights: Vec<(String, f64)> = weights.unwrap_or_default().into_iter().collect();
    let fused = crate::fuse_hits(&hits, k.unwrap_or(rrf::K), &weights, limit);
    list_to_py(py, &fused)
}

/// Say what a file is from its bytes and its name.
#[pyfunction]
fn identify<'py>(py: Python<'py>, name: &str, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
    to_py(py, &crate::identify(name, data))
}

/// Read the references out of a bibliography or a page of notes.
#[pyfunction]
fn find_citations<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyList>> {
    list_to_py(py, &crate::find_citations(text))
}

/// The question to put to a model about the references that did not parse.
///
/// `None` when every one of them settled, because there is nothing to ask.
#[pyfunction]
fn citation_prompt(references: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let references: Vec<citefind::Reference> = from_py_list(references)?;
    Ok(crate::citation_prompt(&references))
}

/// Fill in what did not parse from a model's reply.
#[pyfunction]
fn merge_citation_reply<'py>(
    py: Python<'py>,
    references: &Bound<'py, PyAny>,
    reply: &str,
) -> PyResult<Bound<'py, PyList>> {
    let references: Vec<citefind::Reference> = from_py_list(references)?;
    let merged = crate::merge_citation_reply(&references, reply)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    list_to_py(py, &merged)
}

/// The series a title belongs to, and which volume of it this is.
///
/// `None` when the title says nothing about a series, which is most titles.
#[pyfunction]
#[pyo3(signature = (title, author = None))]
fn series_of<'py>(
    py: Python<'py>,
    title: &str,
    author: Option<&str>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    match crate::series_of(title, author) {
        Some(found) => Ok(Some(to_py(py, &found)?)),
        None => Ok(None),
    }
}

/// Read an identifier out of whatever it arrived wrapped in.
///
/// `None` when the string is not one.
#[pyfunction]
fn parse_identifier<'py>(py: Python<'py>, text: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    match crate::parse_identifier(text) {
        Some(found) => Ok(Some(to_py(py, &found)?)),
        None => Ok(None),
    }
}

/// The module Python imports.
#[pymodule]
fn _catena(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add_function(wrap_pyfunction!(chunk_markdown, module)?)?;
    module.add_function(wrap_pyfunction!(scrub, module)?)?;
    module.add_function(wrap_pyfunction!(count_tokens, module)?)?;
    module.add_function(wrap_pyfunction!(parse_subtitles, module)?)?;
    module.add_function(wrap_pyfunction!(pack_cues, module)?)?;
    module.add_function(wrap_pyfunction!(segment_cues, module)?)?;
    module.add_function(wrap_pyfunction!(cue_windows, module)?)?;
    module.add_function(wrap_pyfunction!(split_collection, module)?)?;
    module.add_function(wrap_pyfunction!(fuse_hits, module)?)?;
    module.add_function(wrap_pyfunction!(identify, module)?)?;
    module.add_function(wrap_pyfunction!(find_citations, module)?)?;
    module.add_function(wrap_pyfunction!(citation_prompt, module)?)?;
    module.add_function(wrap_pyfunction!(merge_citation_reply, module)?)?;
    module.add_function(wrap_pyfunction!(series_of, module)?)?;
    module.add_function(wrap_pyfunction!(parse_identifier, module)?)?;
    Ok(())
}
