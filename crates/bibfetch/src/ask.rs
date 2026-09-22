//! Asking the registers, which is the part that touches the network.
//!
//! Lifted out of the command line so that the Python bindings ask in exactly
//! the same order, with the same retries and the same backoff. Two
//! implementations of "which register answers about this identifier" would
//! drift, and the one nobody runs from a terminal would drift silently.

use std::time::Duration;

use crate::{Error, Record, Source};

/// How to ask: who to say you are, how long to wait, how often to retry.
#[derive(Debug, Clone)]
pub struct Ask {
    /// An address the registers ask for, and answer faster when they get.
    pub polite: Option<String>,
    /// How long to wait on any one register.
    pub timeout: Duration,
    /// How many times to try a register that is unreachable.
    pub attempts: u32,
}

impl Default for Ask {
    fn default() -> Self {
        Self {
            polite: None,
            timeout: crate::TIMEOUT,
            attempts: crate::ATTEMPTS,
        }
    }
}

impl Ask {
    /// An agent configured to ask this way.
    pub fn agent(&self) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            .user_agent(user_agent(self.polite.as_deref()))
            .build()
            .into()
    }
}

/// Ask every register that answers about this kind of identifier, in turn,
/// and stop at the first complete answer.
///
/// An incomplete answer is kept in case nothing better comes: a title with no
/// authors is still more than nothing, and a register being down is only a
/// failure when every register is. `Ok(None)` means no register here answers
/// about that kind at all.
pub fn lookup(
    agent: &ureq::Agent,
    id: &str,
    kind: &str,
    attempts: u32,
) -> Result<Option<Record>, Error> {
    let chain = Source::chain_for_kind(kind);
    if chain.is_empty() {
        return Ok(None);
    }
    let mut best: Option<Record> = None;
    let mut last_error = None;
    for source in chain {
        match ask(agent, *source, id, kind, attempts) {
            Ok(record) => {
                match &mut best {
                    // Merged rather than replaced. The registers answer about
                    // different parts of one book: OpenLibrary's edition record
                    // carries the publisher, the pagination and the language
                    // and names its authors by key, while its search index
                    // carries the authors and not the publisher. Keeping
                    // whichever arrived first throws the other half away.
                    Some(kept) => kept.fill_from(record),
                    None => best = Some(record),
                }
                if best.as_ref().is_some_and(Record::complete) {
                    break;
                }
            }
            Err(why) => last_error = Some(why),
        }
    }
    match best {
        Some(record) => Ok(Some(record)),
        None => {
            Err(last_error.unwrap_or_else(|| Error::Unreachable("no register answered".into())))
        }
    }
}

/// Ask a register, retrying a failure that might not be one.
///
/// Waits a second before the second attempt and two before the third: long
/// enough for a register that is rate-limiting, short enough that one which
/// is simply down does not hold up a pipeline.
fn ask(
    agent: &ureq::Agent,
    source: Source,
    id: &str,
    kind: &str,
    attempts: u32,
) -> Result<crate::Record, Error> {
    let mut last = Error::Unreachable("no attempt was made".into());

    for attempt in 0..attempts.max(1) {
        match fetch(agent, source, id, kind) {
            Ok(record) => return Ok(record),
            // A shape this cannot read will not read any better next time.
            Err(e @ Error::Shape(_)) => return Err(e),
            Err(e) => last = e,
        }
        std::thread::sleep(Duration::from_secs(1 << attempt));
    }
    Err(last)
}

fn fetch(
    agent: &ureq::Agent,
    source: Source,
    id: &str,
    kind: &str,
) -> Result<crate::Record, Error> {
    let response = agent.get(source.url(id)).call();
    let mut response = match response {
        Ok(response) => response,
        // A register that does not have it says so with a status, and that is
        // an answer rather than a failure.
        Err(ureq::Error::StatusCode(404)) => return Ok(crate::missing(id, kind, source)),
        Err(e) => return Err(Error::Unreachable(e.to_string())),
    };
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::Unreachable(e.to_string()))?;
    source.read(id, kind, &body)
}

/// Who is asking.
///
/// The registers ask for an address and answer faster when they get one; a
/// tool that does not say who it is gets the slow pool.
fn user_agent(polite: Option<&str>) -> String {
    let version = env!("CARGO_PKG_VERSION");
    match polite {
        Some(address) => format!("bibfetch/{version} (mailto:{address})"),
        None => format!("bibfetch/{version}"),
    }
}
