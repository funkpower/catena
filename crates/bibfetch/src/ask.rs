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
fn ask(
    agent: &ureq::Agent,
    source: Source,
    id: &str,
    kind: &str,
    attempts: u32,
) -> Result<crate::Record, Error> {
    match get(agent, &source.url(id), attempts)? {
        Some(body) => source.read(id, kind, &body),
        // A register that does not have it says so with a status, and that is
        // an answer rather than a failure.
        None => Ok(crate::missing(id, kind, source)),
    }
}

/// The body at an address, or `None` when the register says it has no such
/// thing; a failure is asked again.
///
/// Waits a second before the second attempt and two before the third: long
/// enough for a register that is rate-limiting, short enough that one which
/// is simply down does not hold up a pipeline. Reading what came back is left
/// to the caller, because an answer this cannot read will not read any better
/// next time.
pub(crate) fn get(agent: &ureq::Agent, url: &str, attempts: u32) -> Result<Option<String>, Error> {
    let attempts = attempts.max(1);
    let mut last = Error::Unreachable("no attempt was made".into());
    for attempt in 0..attempts {
        match once(agent, url) {
            Ok(answer) => return Ok(answer),
            Err(e) => last = e,
        }
        if attempt + 1 < attempts {
            std::thread::sleep(Duration::from_secs(1 << attempt.min(5)));
        }
    }
    Err(last)
}

fn once(agent: &ureq::Agent, url: &str) -> Result<Option<String>, Error> {
    let mut response = match agent.get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(e) => return Err(Error::Unreachable(e.to_string())),
    };
    response
        .body_mut()
        .read_to_string()
        .map(Some)
        .map_err(|e| Error::Unreachable(e.to_string()))
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
