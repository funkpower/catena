//! A book's cover, by its ISBN, from Open Library's covers store.
//!
//! Open Library keeps covers apart from its records, at
//! `covers.openlibrary.org`, and answers by ISBN or by the cover's own id. An
//! ISBN is asked in its thirteen-digit form first and its ten-digit form
//! second, because a book is often filed under only one of them; the cover id
//! of the edition's record is asked last, and only when every form of the
//! ISBN asked was answered and had none.
//!
//! What comes back is checked before it is trusted, because it is written to
//! disk next to a book: an image (JPEG or PNG, by what the bytes are as well
//! as what the server says), no larger than [`MAX_BYTES`], reached in at most
//! [`MAX_REDIRECTS`] redirects, each over HTTPS to Open Library's covers or
//! the Internet Archive, all within [`DEADLINE`].

use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::Error;

/// Where Open Library keeps its covers.
pub const OPEN_LIBRARY_COVERS: &str = "https://covers.openlibrary.org";

/// The largest cover accepted. A large cover from Open Library is a few
/// hundred kilobytes; ten megabytes is something that is not a cover.
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;

/// How many redirects to follow. A cover is served from the Internet
/// Archive two redirects away from the address asked.
pub const MAX_REDIRECTS: u32 = 3;

/// The hosts a redirect may lead to, besides the covers store it began at.
/// Today a cover goes `covers.openlibrary.org`, then `archive.org`, then an
/// `ia*.us.archive.org` storage node.
const REDIRECT_HOSTS: [&str; 2] = ["covers.openlibrary.org", "archive.org"];

/// How long one fetch may take, start to finish: every address, attempt,
/// redirect, body and wait between attempts. One fetch runs at a time in a
/// process, and the clock starts once it is this fetch's turn, so time spent
/// waiting behind another fetch is not counted; a caller sharing the process
/// with other fetches can wait that long again before this one begins.
///
/// Without it the worst case at the defaults ran to about eighteen minutes:
/// three addresses, three attempts, up to four requests an attempt at the
/// agent's timeout each, and a minute's `Retry-After` between attempts.
pub const DEADLINE: Duration = Duration::from_secs(120);

/// How long to leave between two covers asked by ISBN. Open Library allows a
/// hundred every five minutes from one address and refuses the rest with
/// `403`; one every three seconds stays under that however long a run goes.
pub const PACE: Duration = Duration::from_secs(3);

/// The longest a `Retry-After` is honoured for. A register asking for more
/// than this is better treated as down than waited on.
const MAX_WAIT: Duration = Duration::from_secs(60);

/// One request at a time, from the whole process. Open Library asks for
/// this, and limits covers asked by ISBN to a hundred every five minutes
/// from one address; threads asking at once would only meet the limit
/// sooner and all back off together.
///
/// It holds when the last cover was asked for by ISBN, so that [`PACE`] is
/// kept across fetches and threads.
static ONE_AT_A_TIME: Mutex<Option<Instant>> = Mutex::new(None);

/// Which of Open Library's three sizes to ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoverSize {
    /// A thumbnail, about 45 pixels wide.
    Small,
    /// About 180 pixels wide.
    Medium,
    /// The largest Open Library keeps, often the original scan.
    #[default]
    Large,
}

impl CoverSize {
    fn letter(self) -> char {
        match self {
            CoverSize::Small => 'S',
            CoverSize::Medium => 'M',
            CoverSize::Large => 'L',
        }
    }
}

/// A cover, checked to be an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    /// The image.
    pub bytes: Vec<u8>,
    /// `image/jpeg` or `image/png`, which the bytes agree with.
    pub media_type: &'static str,
    /// Which register it came from.
    pub source: &'static str,
    /// The address asked, which is stable; the one that finally served the
    /// image is an archive's storage location and is not.
    pub url: String,
}

impl Cover {
    /// The file extension its media type is written with.
    pub fn extension(&self) -> &'static str {
        match self.media_type {
            "image/png" => "png",
            _ => "jpg",
        }
    }
}

/// Fetch the cover of the book with this ISBN, or of the cover id a record
/// named, at the default settings.
///
/// `Ok(None)` means Open Library has no cover for it. The agent decides the
/// timeout and the User-Agent; [`crate::ask::Ask::agent`] makes one.
pub fn cover(
    agent: &ureq::Agent,
    isbn: &str,
    cover_id: Option<u64>,
    size: CoverSize,
) -> Result<Option<Cover>, Error> {
    Covers::default().fetch(agent, isbn, cover_id, size)
}

/// Where to ask for covers and how hard to try.
#[derive(Debug, Clone)]
pub struct Covers {
    /// The covers store, without a trailing slash.
    pub base: String,
    /// How many times to ask an address that is unreachable or busy.
    pub attempts: u32,
    /// How long the whole fetch may take; [`DEADLINE`] by default. Each
    /// request is given whatever is left of it, or the agent's own timeout
    /// if that is shorter, and a wait that would run past it is not waited.
    pub deadline: Duration,
    /// The least time between two covers asked by ISBN; [`PACE`] by default.
    /// Covers asked by id are not limited and are not spaced out.
    pub pace: Duration,
}

impl Default for Covers {
    fn default() -> Self {
        Self {
            base: OPEN_LIBRARY_COVERS.to_string(),
            attempts: crate::ATTEMPTS,
            deadline: DEADLINE,
            pace: PACE,
        }
    }
}

/// What one address said, short of a failure.
enum Answer {
    Image(Cover),
    Absent,
}

/// Why one attempt at an address failed.
enum Fault {
    /// Worth asking again, after the wait the server asked for if it did.
    Again(Error, Option<Duration>),
    /// Not worth asking again: the answer will be the same.
    Final(Error),
}

impl Covers {
    /// Fetch a cover by ISBN, then by cover id.
    ///
    /// The ISBN may be either form, with or without hyphens. One whose check
    /// digit is wrong is asked about as written and not converted, since a
    /// converted one would be another book's number. A failure at one form
    /// of the ISBN does not stop the other being asked, but it does stop the
    /// cover id being asked: the ISBN's own cover may exist, and is the one
    /// wanted. The failure is returned when no ISBN form gave a cover.
    ///
    /// Refusal for asking too often is the exception. It is
    /// [`Error::Limited`], the other form is not asked, since it would only
    /// be refused too, and the cover id is still asked, because Open Library
    /// does not limit those. A cover found that way is returned; otherwise the
    /// refusal is, so that the caller knows to ask again later.
    pub fn fetch(
        &self,
        agent: &ureq::Agent,
        isbn: &str,
        cover_id: Option<u64>,
        size: CoverSize,
    ) -> Result<Option<Cover>, Error> {
        let digits = idnorm::isbn::digits(isbn);
        if !matches!(digits.len(), 10 | 13) {
            return Err(Error::Invalid("not an ISBN".into()));
        }
        let forms: Vec<String> = match idnorm::isbn::to_13(&digits) {
            Some(thirteen) => std::iter::once(thirteen)
                .chain(idnorm::isbn::to_10(&digits))
                .collect(),
            None => vec![digits],
        };

        let letter = size.letter();
        let addresses: Vec<String> = forms
            .iter()
            .map(|form| format!("{}/b/isbn/{form}-{letter}.jpg?default=false", self.base))
            .collect();

        let mut last_by_isbn = ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(|held| held.into_inner());
        // From here, not from the call: waiting for the lock is time another
        // fetch spent, and counting it would let a busy process starve this
        // one of the time its own requests need. A deadline too long to add
        // to the clock is no deadline at all.
        let now = Instant::now();
        let until = now
            .checked_add(self.deadline)
            .unwrap_or_else(|| now + Duration::from_secs(365 * 24 * 60 * 60));
        let mut failed = None;
        for address in &addresses {
            if let Some(ready) = last_by_isbn.and_then(|at| at.checked_add(self.pace)) {
                let now = Instant::now();
                if ready > now {
                    if ready >= until {
                        return Err(Error::Unreachable(format!(
                            "{address}: past the {:?} deadline, waiting to keep {:?} \
between covers asked by ISBN",
                            self.deadline, self.pace
                        )));
                    }
                    std::thread::sleep(ready - now);
                }
            }
            let answer = self.ask(agent, address, until);
            *last_by_isbn = Some(Instant::now());
            match answer {
                Ok(Answer::Image(cover)) => return Ok(Some(cover)),
                Ok(Answer::Absent) => {}
                Err(why @ Error::Limited(_)) => {
                    failed = Some(why);
                    break;
                }
                Err(why) => failed = Some(why),
            }
        }
        if let Some(why) = failed.take_if(|why| !matches!(why, Error::Limited(_))) {
            return Err(why);
        }
        let Some(id) = cover_id else {
            return failed.map_or(Ok(None), Err);
        };
        let address = format!("{}/b/id/{id}-{letter}.jpg?default=false", self.base);
        match self.ask(agent, &address, until)? {
            Answer::Image(cover) => Ok(Some(cover)),
            Answer::Absent => failed.map_or(Ok(None), Err),
        }
    }

    /// Ask one address, again while it is busy or unreachable.
    ///
    /// Waits as long as a `Retry-After` says, up to a minute, or else a
    /// second and then two, the same as a record lookup does; but not past
    /// `until`, where a wait that would end later is given up at once.
    fn ask(&self, agent: &ureq::Agent, address: &str, until: Instant) -> Result<Answer, Error> {
        let attempts = self.attempts.max(1);
        let mut last = Error::Unreachable("no attempt was made".into());
        for attempt in 0..attempts {
            match self.once(agent, address, until) {
                Ok(answer) => return Ok(answer),
                Err(Fault::Final(why)) => return Err(why),
                Err(Fault::Again(why, wait)) => {
                    if attempt + 1 < attempts {
                        let wait = wait.unwrap_or(Duration::from_secs(1 << attempt.min(5)));
                        if Instant::now() + wait >= until {
                            return Err(self.out_of_time(address, &why));
                        }
                        std::thread::sleep(wait);
                    }
                    last = why;
                }
            }
        }
        Err(last)
    }

    fn out_of_time(&self, address: &str, after: &Error) -> Error {
        Error::Unreachable(format!(
            "{address}: past the {:?} deadline, after {after}",
            self.deadline
        ))
    }

    /// One attempt at an address, following its redirects by hand so that
    /// each one can be checked before it is followed.
    fn once(&self, agent: &ureq::Agent, address: &str, until: Instant) -> Result<Answer, Fault> {
        let mut at = address.to_string();
        let mut hops = 0;
        loop {
            // The agent's timeout bounds one request, body included; what is
            // left of the deadline bounds it too, so no request outlives it.
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Fault::Final(Error::Unreachable(format!(
                    "{at}: past the {:?} deadline",
                    self.deadline
                ))));
            }
            let timeout = agent
                .config()
                .timeouts()
                .global
                .map_or(left, |own| own.min(left));
            let response = agent
                .get(&at)
                .config()
                .max_redirects(0)
                .http_status_as_error(false)
                .timeout_global(Some(timeout))
                .build()
                .call()
                .map_err(|e| Fault::Again(Error::Unreachable(format!("{at}: {e}")), None))?;
            let status = response.status().as_u16();
            match status {
                // A connection lost while reading is worth another try; a body
                // that is not a cover is not.
                200 => {
                    return read(response, address).map_err(|why| match why {
                        Error::Unreachable(_) => Fault::Again(why, None),
                        why => Fault::Final(why),
                    });
                }
                301 | 302 | 303 | 307 | 308 => {
                    let Some(location) = header(&response, "location") else {
                        return Err(Fault::Final(Error::Unreachable(format!(
                            "{at}: a redirect with nowhere to go"
                        ))));
                    };
                    if hops == MAX_REDIRECTS {
                        return Err(Fault::Final(Error::Unreachable(format!(
                            "{address}: more than {MAX_REDIRECTS} redirects"
                        ))));
                    }
                    let next = resolve(&at, &location);
                    if !may_follow(&next, &self.base) {
                        return Err(Fault::Final(Error::Unreachable(format!(
                            "{at}: refused a redirect to {next}, which is not HTTPS to \
Open Library's covers or the Internet Archive"
                        ))));
                    }
                    at = next;
                    hops += 1;
                }
                // `default=false` makes a missing cover a 404 rather than a
                // stand-in image.
                404 | 410 => return Ok(Answer::Absent),
                // Open Library's answer to more than a hundred covers asked by
                // ISBN in five minutes from one address. Asking again at once
                // would only be refused again.
                403 => {
                    return Err(Fault::Final(Error::Limited(format!(
                        "{at}: refused (403); Open Library allows a hundred covers \
by ISBN every five minutes from one address"
                    ))));
                }
                429 | 503 => {
                    let wait = header(&response, "retry-after")
                        .and_then(|value| value.trim().parse::<u64>().ok())
                        .map(|seconds| Duration::from_secs(seconds).min(MAX_WAIT));
                    return Err(Fault::Again(
                        Error::Unreachable(format!("{at}: busy ({status})")),
                        wait,
                    ));
                }
                500..=599 => {
                    return Err(Fault::Again(
                        Error::Unreachable(format!("{at}: answered {status}")),
                        None,
                    ));
                }
                _ => {
                    return Err(Fault::Final(Error::Unreachable(format!(
                        "{at}: answered {status}"
                    ))));
                }
            }
        }
    }
}

/// Whether an image's bytes run to its end marker: `FF D9` for a JPEG, which
/// some encoders follow with zero padding, and the `IEND` chunk for a PNG,
/// which is its last twelve bytes.
fn ends_whole(media_type: &str, bytes: &[u8]) -> bool {
    match media_type {
        "image/jpeg" => {
            let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |at| at + 1);
            end >= 5 && bytes[..end].ends_with(&[0xFF, 0xD9])
        }
        _ => {
            bytes.len() >= 8 + 12
                && bytes[bytes.len() - 12..].starts_with(&[0, 0, 0, 0])
                && &bytes[bytes.len() - 8..bytes.len() - 4] == b"IEND"
        }
    }
}

/// Read and check a body that is meant to be a cover.
fn read(mut response: ureq::http::Response<ureq::Body>, address: &str) -> Result<Answer, Error> {
    let declared = header(&response, "content-type")
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    let media_type = match declared.as_str() {
        "image/jpeg" => "image/jpeg",
        "image/png" => "image/png",
        other => {
            return Err(Error::Shape(format!(
                "{address}: not an image but {}",
                if other.is_empty() { "untyped" } else { other }
            )));
        }
    };
    if response
        .body()
        .content_length()
        .is_some_and(|length| length > MAX_BYTES)
    {
        return Err(Error::Shape(format!(
            "{address}: larger than {MAX_BYTES} bytes"
        )));
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .map_err(|e| match e {
            ureq::Error::BodyExceedsLimit(_) => {
                Error::Shape(format!("{address}: larger than {MAX_BYTES} bytes"))
            }
            e => Error::Unreachable(format!("{address}: {e}")),
        })?;

    let agrees = match media_type {
        "image/jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        _ => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
    };
    if !agrees {
        return Err(Error::Shape(format!(
            "{address}: said {media_type} but is not one"
        )));
    }
    // The start of an image is not an image. A body sent with no length,
    // or a stub, can begin right and stop short; what ends it says it is
    // whole.
    if !ends_whole(media_type, &bytes) {
        return Err(Error::Shape(format!(
            "{address}: a {media_type} that stops before its end"
        )));
    }
    // No floor on the size. Open Library's stand-in for a missing cover is a
    // 43-byte, one-pixel GIF, and it serves that only when `default=false`
    // is left off: with it, as every address here has it, a missing cover is
    // a 404, for an ISBN and for a cover id alike (checked 2026-11-13). The
    // stand-in would fail the media type check above in any case, being a
    // GIF. So a floor could only drop real covers, and real Small ones sit
    // close to any floor worth having: thumbnails measured 1032 to 2148
    // bytes, the smallest eight bytes over a kilobyte floor, so smaller ones
    // not yet seen would have been dropped.
    Ok(Answer::Image(Cover {
        bytes,
        media_type,
        source: "openlibrary",
        url: address.to_string(),
    }))
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

/// Whether a redirect may be followed to `next`.
///
/// Back to the covers store the fetch began at, which the caller chose, or
/// else over HTTPS on the standard port to [`REDIRECT_HOSTS`] or a host under
/// `archive.org`, named and not numbered. A redirect is the server choosing
/// where this process connects next, and an address by number, or a host
/// that is neither Open Library nor the Archive, is somewhere a compromised
/// or mistaken redirect could reach a service on the local network.
fn may_follow(next: &str, base: &str) -> bool {
    if origin(next).is_some() && origin(next) == origin(base) {
        return true;
    }
    // The same parser the request is made with, so the host checked is the
    // host connected to.
    let Ok(uri) = next.parse::<ureq::http::Uri>() else {
        return false;
    };
    if uri.scheme_str() != Some("https") || uri.port_u16().is_some_and(|port| port != 443) {
        return false;
    }
    let Some(authority) = uri.authority() else {
        return false;
    };
    if authority.as_str().contains('@') {
        return false;
    }
    let host = authority.host().to_ascii_lowercase();
    if host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        .is_ok()
    {
        return false;
    }
    REDIRECT_HOSTS.contains(&host.as_str()) || host.ends_with(".archive.org")
}

/// `scheme://host:port` of an absolute URL.
fn origin(url: &str) -> Option<&str> {
    let after = url.find("://")? + 3;
    let end = url[after..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |at| after + at);
    Some(&url[..end])
}

/// Where a `Location` points, read against the address that sent it.
fn resolve(from: &str, location: &str) -> String {
    // Absolute only when the scheme comes first: a relative address can
    // carry another address in its query.
    if location
        .find("://")
        .is_some_and(|at| !location[..at].contains(['/', '?', '#']))
    {
        return location.to_string();
    }
    if let Some(rest) = location.strip_prefix("//") {
        let scheme = from.split("://").next().unwrap_or("https");
        return format!("{scheme}://{rest}");
    }
    let Some(base) = origin(from) else {
        return location.to_string();
    };
    if location.starts_with('/') {
        return format!("{base}{location}");
    }
    let path = &from[base.len()..];
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let directory = path.rfind('/').map_or("/", |at| &path[..=at]);
    format!("{base}{directory}{location}")
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// One reply from the stub: a status, headers and a body.
    struct Reply {
        status: u16,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
        /// How long to sit on the request before answering.
        delay: Duration,
        /// Whether to say how long the body is, or leave the reader to
        /// find out when the connection closes.
        announce: bool,
    }

    impl Reply {
        fn status(status: u16) -> Self {
            Reply {
                status,
                headers: Vec::new(),
                body: Vec::new(),
                delay: Duration::ZERO,
                announce: true,
            }
        }

        fn image(media_type: &str, body: Vec<u8>) -> Self {
            Reply {
                status: 200,
                headers: vec![("Content-Type", media_type.to_string())],
                body,
                delay: Duration::ZERO,
                announce: true,
            }
        }

        fn redirect(to: String) -> Self {
            Reply {
                status: 302,
                headers: vec![("Location", to)],
                body: Vec::new(),
                delay: Duration::ZERO,
                announce: true,
            }
        }

        fn header(mut self, name: &'static str, value: &str) -> Self {
            self.headers.push((name, value.to_string()));
            self
        }
    }

    /// A covers store on a loopback port, answering each path with whatever
    /// the route says, and keeping the paths it was asked for in order.
    ///
    /// Written out here rather than taken from a crate: a test that needs a
    /// socket, a request line and a canned answer needs nothing else.
    struct Stub {
        base: String,
        asked: Arc<Mutex<Vec<String>>>,
    }

    impl Stub {
        fn start(route: impl Fn(&str, &str) -> Reply + Send + Sync + 'static) -> Stub {
            let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
            let base = format!("http://{}", listener.local_addr().expect("an address"));
            let asked = Arc::new(Mutex::new(Vec::new()));
            let route = Arc::new(route);
            let (seen, own) = (Arc::clone(&asked), base.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    let (route, seen, own) = (Arc::clone(&route), Arc::clone(&seen), own.clone());
                    std::thread::spawn(move || {
                        let mut reader = BufReader::new(stream.try_clone().expect("a clone"));
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_err() {
                            return;
                        }
                        let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                        loop {
                            let mut header = String::new();
                            match reader.read_line(&mut header) {
                                Ok(0) | Err(_) => break,
                                Ok(_) if header == "\r\n" => break,
                                Ok(_) => {}
                            }
                        }
                        seen.lock().expect("a lock").push(path.clone());
                        let reply = route(&own, &path);
                        std::thread::sleep(reply.delay);
                        let mut head =
                            format!("HTTP/1.1 {} Stub\r\nConnection: close\r\n", reply.status);
                        let stated = reply
                            .headers
                            .iter()
                            .any(|(name, _)| *name == "Content-Length");
                        if reply.announce && !stated {
                            head.push_str(&format!("Content-Length: {}\r\n", reply.body.len()));
                        }
                        for (name, value) in &reply.headers {
                            head.push_str(&format!("{name}: {value}\r\n"));
                        }
                        head.push_str("\r\n");
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(&reply.body);
                    });
                }
            });
            Stub { base, asked }
        }

        fn covers(&self) -> Covers {
            Covers {
                base: self.base.clone(),
                attempts: 3,
                deadline: DEADLINE,
                pace: Duration::ZERO,
            }
        }

        fn asked(&self) -> Vec<String> {
            self.asked.lock().expect("a lock").clone()
        }
    }

    fn agent() -> ureq::Agent {
        crate::ask::Ask {
            timeout: Duration::from_secs(5),
            ..Default::default()
        }
        .agent()
    }

    /// A JPEG's start, filler, and its end marker, `len` bytes in all.
    fn jpeg(len: usize) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
        bytes.resize(len - 2, 0x11);
        bytes.extend([0xFF, 0xD9]);
        bytes
    }

    /// A PNG's signature, filler, and its `IEND` chunk, `len` bytes in all.
    fn png(len: usize) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.resize(len - 12, 0x22);
        bytes.extend(b"\0\0\0\0IEND\xAE\x42\x60\x82");
        bytes
    }

    const ISBN13: &str = "/b/isbn/9780306406157-L.jpg?default=false";
    const ISBN10: &str = "/b/isbn/0306406152-L.jpg?default=false";

    #[test]
    fn a_cover_is_found_by_its_isbn() {
        let stub = Stub::start(|_, path| match path {
            ISBN13 => Reply::image("image/jpeg", jpeg(4096)),
            _ => Reply::status(404),
        });
        let cover = stub
            .covers()
            .fetch(&agent(), "978-0-306-40615-7", None, CoverSize::Large)
            .expect("an answer")
            .expect("a cover");
        assert_eq!(cover.media_type, "image/jpeg");
        assert_eq!(cover.extension(), "jpg");
        assert_eq!(cover.source, "openlibrary");
        assert_eq!(cover.bytes, jpeg(4096));
        assert_eq!(cover.url, format!("{}{ISBN13}", stub.base));
        assert_eq!(stub.asked(), [ISBN13]);
    }

    #[test]
    fn a_png_cover_is_a_cover_too() {
        let stub = Stub::start(|_, _| Reply::image("image/png", png(2048)));
        let cover = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Medium)
            .expect("an answer")
            .expect("a cover");
        assert_eq!(cover.media_type, "image/png");
        assert_eq!(cover.extension(), "png");
        assert_eq!(stub.asked(), ["/b/isbn/9780306406157-M.jpg?default=false"]);
    }

    #[test]
    fn no_cover_anywhere_is_none_and_not_a_failure() {
        let stub = Stub::start(|_, _| Reply::status(404));
        let found = stub
            .covers()
            .fetch(&agent(), "9780306406157", Some(42), CoverSize::Large)
            .expect("an answer");
        assert_eq!(found, None);
        // Both forms of the ISBN, then the cover id, and each only once.
        assert_eq!(
            stub.asked(),
            [ISBN13, ISBN10, "/b/id/42-L.jpg?default=false"]
        );
    }

    #[test]
    fn a_book_filed_under_its_ten_digit_isbn_is_still_found() {
        let stub = Stub::start(|_, path| match path {
            ISBN10 => Reply::image("image/jpeg", jpeg(2048)),
            _ => Reply::status(404),
        });
        // Asked with the ten-digit form, which is tried second.
        let cover = stub
            .covers()
            .fetch(&agent(), "0-306-40615-2", None, CoverSize::Large)
            .expect("an answer")
            .expect("a cover");
        assert_eq!(cover.url, format!("{}{ISBN10}", stub.base));
        assert_eq!(stub.asked(), [ISBN13, ISBN10]);
    }

    #[test]
    fn a_cover_id_from_the_record_is_asked_when_the_isbn_has_none() {
        let stub = Stub::start(|_, path| match path {
            "/b/id/1162784-L.jpg?default=false" => Reply::image("image/jpeg", jpeg(2048)),
            _ => Reply::status(404),
        });
        let cover = stub
            .covers()
            .fetch(&agent(), "9780306406157", Some(1162784), CoverSize::Large)
            .expect("an answer")
            .expect("a cover");
        assert!(cover.url.ends_with("/b/id/1162784-L.jpg?default=false"));
    }

    #[test]
    fn a_mistyped_isbn_is_asked_as_written_and_not_converted() {
        let stub = Stub::start(|_, _| Reply::status(404));
        let found = stub
            .covers()
            .fetch(&agent(), "978-0-306-40615-8", None, CoverSize::Large)
            .expect("an answer");
        assert_eq!(found, None);
        assert_eq!(stub.asked(), ["/b/isbn/9780306406158-L.jpg?default=false"]);
    }

    #[test]
    fn something_that_is_not_an_isbn_is_refused_without_asking() {
        let stub = Stub::start(|_, _| Reply::status(404));
        let failed = stub
            .covers()
            .fetch(&agent(), "12345", None, CoverSize::Large)
            .expect_err("not an isbn");
        assert!(matches!(failed, Error::Invalid(_)), "{failed:?}");
        assert!(stub.asked().is_empty());
    }

    #[test]
    fn open_librarys_stand_in_is_never_a_cover() {
        // What Open Library serves for a missing cover when `default=false`
        // is left off: a one-pixel GIF89a, 43 bytes.
        let mut gif = b"GIF89a\x01\x00\x01\x00".to_vec();
        gif.resize(43, 0);
        let stub = Stub::start(move |_, _| Reply::image("image/gif", gif.clone()));
        let failed = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Small)
            .expect_err("not a cover");
        assert!(matches!(failed, Error::Shape(_)), "{failed:?}");
    }

    #[test]
    fn a_page_that_is_not_an_image_is_refused() {
        let stub = Stub::start(|_, _| Reply::image("text/html; charset=utf-8", jpeg(4096)));
        let failed = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect_err("not an image");
        assert!(matches!(failed, Error::Shape(_)), "{failed:?}");
    }

    #[test]
    fn bytes_that_are_not_the_image_they_claim_are_refused() {
        let stub = Stub::start(|_, _| Reply::image("image/jpeg", b"<html>".repeat(500)));
        let failed = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect_err("not a jpeg");
        assert!(matches!(failed, Error::Shape(_)), "{failed:?}");

        let stub = Stub::start(|_, _| Reply::image("image/png", jpeg(4096)));
        let failed = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect_err("a jpeg is not a png");
        assert!(matches!(failed, Error::Shape(_)), "{failed:?}");
    }

    #[test]
    fn a_body_over_the_limit_is_refused() {
        // Announced, and refused on the length; then not announced, so the
        // cap has to hold while reading.
        for announce in [true, false] {
            let stub = Stub::start(move |_, _| Reply {
                announce,
                ..Reply::image("image/jpeg", jpeg(MAX_BYTES as usize + 1))
            });
            let failed = stub
                .covers()
                .fetch(&agent(), "9780306406157", None, CoverSize::Large)
                .expect_err("too large");
            assert!(
                matches!(&failed, Error::Shape(why) if why.contains("larger than")),
                "announce {announce}: {failed:?}"
            );
        }
    }

    #[test]
    fn a_body_of_unannounced_length_under_the_limit_is_read_whole() {
        let stub = Stub::start(|_, _| Reply {
            announce: false,
            ..Reply::image("image/jpeg", jpeg(64 * 1024))
        });
        let cover = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect("an answer")
            .expect("a cover");
        assert_eq!(cover.bytes.len(), 64 * 1024);
    }

    #[test]
    fn a_busy_register_is_waited_on_and_asked_again() {
        let calls = Arc::new(Mutex::new(0u32));
        let count = Arc::clone(&calls);
        let stub = Stub::start(move |_, _| {
            let mut n = count.lock().expect("a lock");
            *n += 1;
            match *n {
                1 => Reply::status(429).header("Retry-After", "0"),
                2 => Reply::status(503).header("Retry-After", "0"),
                _ => Reply::image("image/jpeg", jpeg(2048)),
            }
        });
        let cover = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect("an answer");
        assert!(cover.is_some());
        assert_eq!(stub.asked(), [ISBN13, ISBN13, ISBN13]);
    }

    #[test]
    fn a_body_cut_short_is_asked_for_again() {
        let calls = Arc::new(Mutex::new(0u32));
        let count = Arc::clone(&calls);
        let stub = Stub::start(move |_, _| {
            let mut n = count.lock().expect("a lock");
            *n += 1;
            match *n {
                // Promises more than it sends, then closes.
                1 => Reply::image("image/jpeg", jpeg(2048)).header("Content-Length", "9000"),
                _ => Reply::image("image/jpeg", jpeg(2048)),
            }
        });
        let covers = Covers {
            attempts: 2,
            ..stub.covers()
        };
        let cover = covers
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect("an answer")
            .expect("a cover");
        assert_eq!(cover.bytes.len(), 2048);
        assert_eq!(stub.asked(), [ISBN13, ISBN13]);
    }

    #[test]
    fn a_register_busy_every_time_is_a_failure_and_not_an_absence() {
        let stub = Stub::start(|_, _| Reply::status(429).header("Retry-After", "0"));
        let covers = Covers {
            attempts: 2,
            ..stub.covers()
        };
        let failed = covers
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect_err("busy throughout");
        assert!(matches!(failed, Error::Unreachable(_)), "{failed:?}");
        // Two attempts at each form of the ISBN.
        assert_eq!(stub.asked(), [ISBN13, ISBN13, ISBN10, ISBN10]);
    }

    #[test]
    fn redirects_are_followed_to_the_image() {
        let stub = Stub::start(|base, path| match path {
            ISBN13 => Reply::redirect(format!("{base}/download/one")),
            "/download/one" => Reply::redirect("/view?file=two".to_string()),
            "/view?file=two" => Reply::image("image/jpeg", jpeg(2048)),
            _ => Reply::status(404),
        });
        let cover = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect("an answer")
            .expect("a cover");
        // The address asked, not where the archive happened to serve it.
        assert_eq!(cover.url, format!("{}{ISBN13}", stub.base));
    }

    #[test]
    fn a_fourth_redirect_is_not_followed() {
        let stub = Stub::start(|_, path| {
            let n: u32 = path
                .strip_prefix("/hop/")
                .map_or(0, |n| n.parse().unwrap_or(0));
            Reply::redirect(format!("/hop/{}", n + 1))
        });
        let failed = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect_err("too many redirects");
        assert!(
            matches!(&failed, Error::Unreachable(why) if why.contains("redirects")),
            "{failed:?}"
        );
        // Asked, then three redirects followed, for each ISBN form.
        assert_eq!(stub.asked().iter().filter(|p| *p == ISBN13).count(), 1);
        assert_eq!(stub.asked().len(), 8);
    }

    #[test]
    fn a_redirect_off_https_is_not_followed() {
        let stub = Stub::start(|_, path| match path {
            ISBN13 | ISBN10 => Reply::redirect("http://covers.example.invalid/x.jpg".to_string()),
            _ => Reply::status(404),
        });
        let failed = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect_err("plain http elsewhere");
        assert!(
            matches!(&failed, Error::Unreachable(why) if why.contains("refused")),
            "{failed:?}"
        );
    }

    #[test]
    fn redirects_go_only_to_open_library_and_the_archive_over_https() {
        let base = "https://covers.openlibrary.org";
        for next in [
            "https://covers.openlibrary.org/b/id/1-L.jpg",
            "https://archive.org/download/l_covers_0001/x.jpg",
            "https://ia800100.us.archive.org/view_archive.php?f=1",
            "https://ARCHIVE.org/x",
            "https://archive.org:443/x",
        ] {
            assert!(may_follow(next, base), "{next}");
        }
        for next in [
            "http://archive.org/x",
            "https://archive.org:8443/x",
            "https://evilarchive.org/x",
            "https://archive.org.example/x",
            "https://openlibrary.org.example/x",
            "https://covers.openlibrary.org@127.0.0.1/x",
            "https://127.0.0.1/x",
            "https://10.7.1.5/x",
            "https://[::1]/x",
            "https://example.com/x",
            "ftp://archive.org/x",
        ] {
            assert!(!may_follow(next, base), "{next}");
        }
        // The store it began at, whatever it is, for a test or a mirror.
        assert!(may_follow("http://127.0.0.1:9/x", "http://127.0.0.1:9"));
        assert!(!may_follow("http://127.0.0.1:10/x", "http://127.0.0.1:9"));
    }

    #[test]
    fn a_register_that_does_not_answer_in_time_is_unreachable() {
        let stub = Stub::start(|_, _| Reply {
            delay: Duration::from_secs(3),
            ..Reply::image("image/jpeg", jpeg(2048))
        });
        let agent = crate::ask::Ask {
            timeout: Duration::from_millis(300),
            ..Default::default()
        }
        .agent();
        let covers = Covers {
            attempts: 1,
            ..stub.covers()
        };
        // No stopwatch: one started here would count the wait for the
        // process lock, which other tests hold. The failure is proof enough,
        // since a reply waited out is a cover.
        let failed = covers
            .fetch(&agent, "9780306406157", None, CoverSize::Large)
            .expect_err("timed out");
        assert!(matches!(failed, Error::Unreachable(_)), "{failed:?}");
    }

    #[test]
    fn a_slow_answer_is_cut_at_the_deadline() {
        let stub = Stub::start(|_, _| Reply {
            delay: Duration::from_secs(10),
            ..Reply::image("image/jpeg", jpeg(2048))
        });
        // The agent would wait the reply out; the deadline will not.
        let agent = crate::ask::Ask {
            timeout: Duration::from_secs(30),
            ..Default::default()
        }
        .agent();
        let covers = Covers {
            attempts: 3,
            deadline: Duration::from_millis(500),
            ..stub.covers()
        };
        let found = covers
            .fetch(&agent, "9780306406157", Some(42), CoverSize::Large)
            .map(|cover| cover.map(|cover| cover.url));
        assert!(matches!(found, Err(Error::Unreachable(_))), "{found:?}");
        // Out of time after the first request: no second attempt, no other
        // form of the ISBN, no cover id.
        assert_eq!(stub.asked(), [ISBN13]);
    }

    #[test]
    fn a_wait_past_the_deadline_is_not_waited() {
        let stub = Stub::start(|_, _| Reply::status(429).header("Retry-After", "30"));
        let covers = Covers {
            attempts: 3,
            deadline: Duration::from_secs(5),
            ..stub.covers()
        };
        let started = Instant::now();
        let failed = covers
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect_err("busy");
        assert!(
            matches!(&failed, Error::Unreachable(why) if why.contains("deadline")),
            "{failed:?}"
        );
        // Each form asked once and given up on, not waited on for 30 s. The
        // bound is loose because it includes any wait for the process lock.
        assert_eq!(stub.asked(), [ISBN13, ISBN10]);
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn an_isbn_that_fails_stops_the_cover_id_being_asked() {
        // The ISBN-13 address fails (not a 404), the ISBN-10 has none, and
        // the cover id has an image that might be another edition's.
        let stub = Stub::start(|_, path| match path {
            ISBN13 => Reply::status(500),
            "/b/id/42-L.jpg?default=false" => Reply::image("image/jpeg", jpeg(4096)),
            _ => Reply::status(404),
        });
        let covers = Covers {
            attempts: 1,
            ..stub.covers()
        };
        // Only the address of anything found, so a failure prints short.
        let found = covers
            .fetch(&agent(), "9780306406157", Some(42), CoverSize::Large)
            .map(|cover| cover.map(|cover| cover.url));
        assert!(matches!(found, Err(Error::Unreachable(_))), "{found:?}");
        assert_eq!(stub.asked(), [ISBN13, ISBN10]);
    }

    #[test]
    fn a_refusal_for_asking_too_often_still_asks_the_cover_id() {
        let stub = Stub::start(|_, path| match path {
            ISBN13 => Reply::status(403),
            "/b/id/42-L.jpg?default=false" => Reply::image("image/jpeg", jpeg(4096)),
            _ => Reply::status(404),
        });
        let cover = stub
            .covers()
            .fetch(&agent(), "9780306406157", Some(42), CoverSize::Large)
            .expect("an answer")
            .expect("a cover");
        assert!(cover.url.ends_with("/b/id/42-L.jpg?default=false"));
        // The ten-digit form would only have been refused too.
        assert_eq!(stub.asked(), [ISBN13, "/b/id/42-L.jpg?default=false"]);
    }

    #[test]
    fn a_refusal_for_asking_too_often_is_not_an_absence() {
        let stub = Stub::start(|_, path| match path {
            ISBN13 => Reply::status(403),
            _ => Reply::status(404),
        });
        let found = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .map(|cover| cover.map(|cover| cover.url));
        assert!(matches!(found, Err(Error::Limited(_))), "{found:?}");
        assert_eq!(stub.asked(), [ISBN13]);

        // With a cover id that has no cover either, the refusal still stands:
        // the ISBN's own cover may be there once the limit passes.
        let found = stub
            .covers()
            .fetch(&agent(), "9780306406157", Some(42), CoverSize::Large)
            .map(|cover| cover.map(|cover| cover.url));
        assert!(matches!(found, Err(Error::Limited(_))), "{found:?}");
    }

    #[test]
    fn a_deadline_too_long_to_count_is_no_deadline() {
        let stub = Stub::start(|_, _| Reply::image("image/jpeg", jpeg(2048)));
        let covers = Covers {
            deadline: Duration::MAX,
            ..stub.covers()
        };
        let cover = covers
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect("an answer");
        assert!(cover.is_some());
    }

    #[test]
    fn covers_asked_by_isbn_are_spaced_out() {
        let stub = Stub::start(|_, _| Reply::status(404));
        let covers = Covers {
            pace: Duration::from_millis(300),
            ..stub.covers()
        };
        let started = Instant::now();
        let found = covers.fetch(&agent(), "9780306406157", None, CoverSize::Large);
        assert!(matches!(found, Ok(None)), "{found:?}");
        // Two forms of the ISBN, so at least one pause between them.
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert_eq!(stub.asked(), [ISBN13, ISBN10]);
    }

    #[test]
    fn a_magic_number_alone_is_not_a_cover() {
        let mut cut_jpeg = jpeg(4096);
        cut_jpeg.truncate(4000);
        let mut cut_png = png(4096);
        cut_png.truncate(4000);
        for (media_type, body) in [
            ("image/jpeg", vec![0xFF, 0xD8, 0xFF]),
            ("image/png", b"\x89PNG\r\n\x1a\n".to_vec()),
            ("image/jpeg", cut_jpeg),
            ("image/png", cut_png),
        ] {
            let stub = Stub::start(move |_, _| Reply::image(media_type, body.clone()));
            let found = stub
                .covers()
                .fetch(&agent(), "9780306406157", None, CoverSize::Large)
                .map(|cover| cover.map(|cover| cover.bytes.len()));
            assert!(
                matches!(found, Err(Error::Shape(_))),
                "{media_type}: {found:?}"
            );
        }
    }

    #[test]
    fn a_jpeg_padded_after_its_end_marker_is_whole() {
        let mut padded = jpeg(2048);
        padded.extend([0, 0, 0]);
        assert!(ends_whole("image/jpeg", &padded));
        assert!(!ends_whole("image/jpeg", &[0xFF, 0xD8, 0xFF, 0, 0]));
    }

    #[test]
    fn a_redirect_to_an_address_by_number_is_refused() {
        // A listener standing in for a service on the LAN; it only counts.
        let lan = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = lan.local_addr().expect("an address").port();
        let reached = Arc::new(Mutex::new(0u32));
        let count = Arc::clone(&reached);
        std::thread::spawn(move || {
            for stream in lan.incoming() {
                if stream.is_ok() {
                    *count.lock().expect("a lock") += 1;
                }
            }
        });
        let stub =
            Stub::start(move |_, _| Reply::redirect(format!("https://127.0.0.1:{port}/admin")));
        let failed = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect_err("refused");
        assert!(
            matches!(&failed, Error::Unreachable(why) if why.contains("refused")),
            "{failed:?}"
        );
        assert_eq!(*reached.lock().expect("a lock"), 0);
    }

    #[test]
    fn a_small_thumbnail_is_a_cover() {
        // Real Small covers run from about a kilobyte; one under it is
        // still a cover.
        let stub = Stub::start(|_, _| Reply::image("image/jpeg", jpeg(700)));
        let cover = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Small)
            .expect("an answer")
            .expect("a cover");
        assert_eq!(cover.bytes.len(), 700);
    }

    #[test]
    fn locations_are_read_against_the_address_that_sent_them() {
        let from = "https://a.example/b/isbn/1-L.jpg?default=false";
        assert_eq!(resolve(from, "https://c.example/x"), "https://c.example/x");
        assert_eq!(resolve(from, "//c.example/x"), "https://c.example/x");
        assert_eq!(resolve(from, "/x?y=1"), "https://a.example/x?y=1");
        assert_eq!(resolve(from, "x.jpg"), "https://a.example/b/isbn/x.jpg");
        // An address in a relative one's query does not make it absolute.
        assert_eq!(
            resolve(from, "/view_archive.php?from=https://archive.org/x"),
            "https://a.example/view_archive.php?from=https://archive.org/x"
        );
        assert_eq!(
            resolve(from, "x?u=https://c.example/"),
            "https://a.example/b/isbn/x?u=https://c.example/"
        );
        assert_eq!(
            origin("http://127.0.0.1:8080/a?b"),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(origin("https://a.example"), Some("https://a.example"));
    }

    /// Run by hand: `cargo test -p bibfetch -- --ignored`.
    #[test]
    #[ignore = "asks Open Library over the network"]
    fn a_known_book_has_a_cover_at_open_library() {
        let agent = crate::ask::Ask::default().agent();
        // Error-correction coding for digital communications, Plenum 1981.
        let cover = cover(&agent, "0-306-40615-2", None, CoverSize::Large)
            .expect("an answer")
            .expect("a cover");
        assert_eq!(cover.media_type, "image/jpeg");
        assert!(cover.bytes.len() > 1024);
        assert!(cover.url.starts_with(OPEN_LIBRARY_COVERS));
    }
}
