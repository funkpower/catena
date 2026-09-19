//! Lines in, vectors out, against an HTTP embedding endpoint.
//!
//! The one tool here that touches the network, and it does so for a reason:
//! an embedding model is half a gigabyte of weights and a GPU, which belongs
//! behind a server rather than inside a filter. What this owns is everything
//! around the call — batching, the defensive truncation, the retry, and the
//! assertion that the vectors coming back are the width the index expects.
//!
//! The request shape is the one Ollama's `/api/embed` uses, which several
//! other servers also speak.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// How many texts go in one request.
pub const BATCH: usize = 32;

/// Tokens a single input is cut to before it is sent.
///
/// A defensive cap, not a model limit. A server returns an error for an
/// input past its context length and that error fails the whole batch, so one
/// oversized passage costs thirty-one good ones. Counted with a tokeniser
/// that is not the model's — it is within ten or fifteen per cent, which is
/// all a safety net needs to be.
pub const MAX_INPUT_TOKENS: usize = 1800;

/// How long to wait for a response.
///
/// Generous, because a cold server loading a model onto a GPU can take
/// minutes before it answers at all.
pub const TIMEOUT: Duration = Duration::from_secs(600);

/// How many times a failed batch is tried again.
pub const ATTEMPTS: u32 = 3;

/// What the endpoint is asked.
#[derive(Debug, Serialize)]
struct Request<'a> {
    model: &'a str,
    input: &'a [String],
}

/// What it answers.
#[derive(Debug, Deserialize)]
struct Response {
    embeddings: Vec<Vec<f64>>,
}

/// Why a batch could not be embedded.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The server could not be reached, or did not answer well.
    #[error("{0}")]
    Http(String),
    /// The answer was not the shape this expects.
    #[error("the server's answer was not an embedding: {0}")]
    Shape(String),
    /// The vectors are not the width the caller asked for.
    #[error("vectors are {found} wide, expected {expected}")]
    Width {
        /// What came back.
        found: usize,
        /// What was asked for.
        expected: usize,
    },
    /// The server answered with fewer vectors than it was given texts.
    #[error("sent {sent} texts and got {received} vectors back")]
    Count {
        /// How many texts went out.
        sent: usize,
        /// How many vectors came back.
        received: usize,
    },
}

/// Where and how to embed.
#[derive(Debug, Clone)]
pub struct Endpoint {
    /// The server's base URL, without a trailing slash.
    pub host: String,
    /// The model to ask for.
    pub model: String,
    /// The width the vectors must be, when the caller knows it.
    pub dim: Option<usize>,
    /// Texts per request.
    pub batch: usize,
    /// Tokens per text, or `None` to send them whole.
    pub max_input_tokens: Option<usize>,
    /// How long to wait for one request.
    pub timeout: Duration,
    /// How many times to try a failing batch.
    pub attempts: u32,
}

impl Endpoint {
    /// An endpoint with the usual settings.
    pub fn new(host: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            host: host.into().trim_end_matches('/').to_string(),
            model: model.into(),
            dim: None,
            batch: BATCH,
            max_input_tokens: Some(MAX_INPUT_TOKENS),
            timeout: TIMEOUT,
            attempts: ATTEMPTS,
        }
    }

    /// The URL a request goes to.
    pub fn url(&self) -> String {
        format!("{}/api/embed", self.host)
    }

    /// Cut a text to the input cap, if there is one.
    pub fn truncate(&self, text: &str) -> String {
        match self.max_input_tokens {
            None | Some(0) => text.to_string(),
            Some(limit) => {
                let tokens = textkit::encode(text);
                if tokens.len() <= limit {
                    text.to_string()
                } else {
                    textkit::decode_lossy(&tokens[..limit])
                }
            }
        }
    }
}

/// Embed one batch, retrying a failure.
///
/// Waits one second before the second attempt and two before the third, which
/// is long enough for a server that is loading a model and short enough that
/// a server that is simply down does not hold up a pipeline for a minute.
pub fn embed_batch(
    agent: &ureq::Agent,
    endpoint: &Endpoint,
    texts: &[String],
) -> Result<Vec<Vec<f64>>, Error> {
    let sent: Vec<String> = texts.iter().map(|text| endpoint.truncate(text)).collect();
    let mut last = Error::Http("no attempt was made".into());

    for attempt in 0..endpoint.attempts.max(1) {
        match post(agent, endpoint, &sent) {
            Ok(vectors) => return check(endpoint, &sent, vectors),
            Err(e @ Error::Width { .. } | e @ Error::Count { .. }) => return Err(e),
            Err(e) => last = e,
        }
        std::thread::sleep(Duration::from_secs(1 << attempt));
    }
    Err(last)
}

fn post(
    agent: &ureq::Agent,
    endpoint: &Endpoint,
    texts: &[String],
) -> Result<Vec<Vec<f64>>, Error> {
    let request = Request {
        model: &endpoint.model,
        input: texts,
    };
    let mut response = agent
        .post(endpoint.url())
        .send_json(&request)
        .map_err(|e| Error::Http(e.to_string()))?;
    let body: Response = response
        .body_mut()
        .read_json()
        .map_err(|e| Error::Shape(e.to_string()))?;
    Ok(body.embeddings)
}

fn check(
    endpoint: &Endpoint,
    sent: &[String],
    vectors: Vec<Vec<f64>>,
) -> Result<Vec<Vec<f64>>, Error> {
    if vectors.len() != sent.len() {
        return Err(Error::Count {
            sent: sent.len(),
            received: vectors.len(),
        });
    }
    if let (Some(expected), Some(first)) = (endpoint.dim, vectors.first())
        && first.len() != expected
    {
        return Err(Error::Width {
            found: first.len(),
            expected,
        });
    }
    Ok(vectors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint() -> Endpoint {
        Endpoint::new("http://example.invalid/", "a-model")
    }

    #[test]
    fn a_trailing_slash_does_not_double_up() {
        assert_eq!(endpoint().url(), "http://example.invalid/api/embed");
        assert_eq!(
            Endpoint::new("http://example.invalid", "m").url(),
            "http://example.invalid/api/embed"
        );
    }

    #[test]
    fn a_short_text_is_sent_whole() {
        let text = "the mill stood at the northern end";
        assert_eq!(endpoint().truncate(text), text);
    }

    #[test]
    fn a_long_text_is_cut_to_the_cap() {
        let mut endpoint = endpoint();
        endpoint.max_input_tokens = Some(4);
        let cut = endpoint.truncate(&"word ".repeat(100));
        assert_eq!(textkit::count(&cut), 4);
    }

    #[test]
    fn the_cap_can_be_turned_off() {
        let mut endpoint = endpoint();
        endpoint.max_input_tokens = None;
        let long = "word ".repeat(5000);
        assert_eq!(endpoint.truncate(&long), long);
    }

    #[test]
    fn a_width_that_is_not_the_one_asked_for_is_refused() {
        let mut endpoint = endpoint();
        endpoint.dim = Some(1024);
        let sent = vec!["a".to_string()];
        let e = check(&endpoint, &sent, vec![vec![0.0; 768]]).expect_err("should refuse");
        assert!(matches!(
            e,
            Error::Width {
                found: 768,
                expected: 1024
            }
        ));
    }

    #[test]
    fn a_missing_vector_is_refused() {
        let sent = vec!["a".to_string(), "b".to_string()];
        let e = check(&endpoint(), &sent, vec![vec![0.0; 8]]).expect_err("should refuse");
        assert!(matches!(
            e,
            Error::Count {
                sent: 2,
                received: 1
            }
        ));
    }

    #[test]
    fn vectors_of_the_right_shape_come_straight_back() {
        let mut endpoint = endpoint();
        endpoint.dim = Some(3);
        let sent = vec!["a".to_string()];
        let vectors = vec![vec![1.0, 2.0, 3.0]];
        assert_eq!(
            check(&endpoint, &sent, vectors.clone()).expect("should pass"),
            vectors
        );
    }
}
