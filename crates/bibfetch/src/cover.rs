//! A book's cover, by its ISBN, from Open Library's covers store.
//!
//! Open Library keeps covers apart from its records, at
//! `covers.openlibrary.org`, and answers by ISBN or by the cover's own id. An
//! ISBN is asked in its thirteen-digit form first and its ten-digit form
//! second, because a book is often filed under only one of them; the cover id
//! a record already named is asked last, since it may be the cover of another
//! edition of the work rather than of this one.
//!
//! What comes back is checked before it is trusted, because it is written to
//! disk next to a book: an image (JPEG or PNG, by what the bytes are as well
//! as what the server says), no larger than [`MAX_BYTES`], reached in at most
//! [`MAX_REDIRECTS`] redirects that never leave HTTPS.

use std::sync::Mutex;
use std::time::Duration;

use crate::Error;

/// Where Open Library keeps its covers.
pub const OPEN_LIBRARY_COVERS: &str = "https://covers.openlibrary.org";

/// The largest cover accepted. A large cover from Open Library is a few
/// hundred kilobytes; ten megabytes is something that is not a cover.
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;

/// The smallest cover accepted. Open Library's stand-in for a missing cover
/// is a one-pixel image of a few dozen bytes, and nothing under a kilobyte
/// is a picture of a book.
pub const MIN_BYTES: usize = 1024;

/// How many redirects to follow. A cover is served from the Internet
/// Archive two redirects away from the address asked.
pub const MAX_REDIRECTS: u32 = 3;

/// The longest a `Retry-After` is honoured for. A register asking for more
/// than this is better treated as down than waited on.
const MAX_WAIT: Duration = Duration::from_secs(60);

/// One request at a time, from the whole process. Open Library asks for
/// this, and limits covers asked by ISBN to a hundred every five minutes
/// from one address; threads asking at once would only meet the limit
/// sooner and all back off together.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

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
}

impl Default for Covers {
    fn default() -> Self {
        Self {
            base: OPEN_LIBRARY_COVERS.to_string(),
            attempts: crate::ATTEMPTS,
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
    /// converted one would be another book's number. A failure at one address
    /// does not stop the next being asked; it is returned only when no
    /// address gave a cover.
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
        let mut addresses: Vec<String> = forms
            .iter()
            .map(|form| format!("{}/b/isbn/{form}-{letter}.jpg?default=false", self.base))
            .collect();
        if let Some(id) = cover_id {
            addresses.push(format!(
                "{}/b/id/{id}-{letter}.jpg?default=false",
                self.base
            ));
        }

        let _turn = ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(|held| held.into_inner());
        let mut failed = None;
        for address in addresses {
            match self.ask(agent, &address) {
                Ok(Answer::Image(cover)) => return Ok(Some(cover)),
                Ok(Answer::Absent) => {}
                Err(why) => failed = Some(why),
            }
        }
        match failed {
            Some(why) => Err(why),
            None => Ok(None),
        }
    }

    /// Ask one address, again while it is busy or unreachable.
    ///
    /// Waits as long as a `Retry-After` says, up to a minute, or else a
    /// second and then two, the same as a record lookup does.
    fn ask(&self, agent: &ureq::Agent, address: &str) -> Result<Answer, Error> {
        let attempts = self.attempts.max(1);
        let mut last = Error::Unreachable("no attempt was made".into());
        for attempt in 0..attempts {
            match self.once(agent, address) {
                Ok(answer) => return Ok(answer),
                Err(Fault::Final(why)) => return Err(why),
                Err(Fault::Again(why, wait)) => {
                    last = why;
                    if attempt + 1 < attempts {
                        std::thread::sleep(
                            wait.unwrap_or(Duration::from_secs(1 << attempt.min(5))),
                        );
                    }
                }
            }
        }
        Err(last)
    }

    /// One attempt at an address, following its redirects by hand so that
    /// each one can be checked before it is followed.
    fn once(&self, agent: &ureq::Agent, address: &str) -> Result<Answer, Fault> {
        let mut at = address.to_string();
        let mut hops = 0;
        loop {
            let response = agent
                .get(&at)
                .config()
                .max_redirects(0)
                .http_status_as_error(false)
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
                    if !next.starts_with("https://") && origin(&next) != origin(&self.base) {
                        return Err(Fault::Final(Error::Unreachable(format!(
                            "{at}: refused a redirect off HTTPS, to {next}"
                        ))));
                    }
                    at = next;
                    hops += 1;
                }
                // `default=false` makes a missing cover a 404 rather than a
                // stand-in image.
                404 | 410 => return Ok(Answer::Absent),
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
    if bytes.len() < MIN_BYTES {
        return Ok(Answer::Absent);
    }
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
    if location.contains("://") {
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

    fn jpeg(len: usize) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
        bytes.resize(len, 0x11);
        bytes
    }

    fn png(len: usize) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.resize(len, 0x22);
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
    fn a_placeholder_sized_image_is_no_cover() {
        let stub = Stub::start(|_, _| Reply::image("image/jpeg", jpeg(MIN_BYTES - 1)));
        let found = stub
            .covers()
            .fetch(&agent(), "9780306406157", None, CoverSize::Large)
            .expect("an answer");
        assert_eq!(found, None);
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
            matches!(&failed, Error::Unreachable(why) if why.contains("off HTTPS")),
            "{failed:?}"
        );
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
        let started = std::time::Instant::now();
        let failed = covers
            .fetch(&agent, "9780306406157", None, CoverSize::Large)
            .expect_err("timed out");
        assert!(matches!(failed, Error::Unreachable(_)), "{failed:?}");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "waited the reply out"
        );
    }

    #[test]
    fn locations_are_read_against_the_address_that_sent_them() {
        let from = "https://a.example/b/isbn/1-L.jpg?default=false";
        assert_eq!(resolve(from, "https://c.example/x"), "https://c.example/x");
        assert_eq!(resolve(from, "//c.example/x"), "https://c.example/x");
        assert_eq!(resolve(from, "/x?y=1"), "https://a.example/x?y=1");
        assert_eq!(resolve(from, "x.jpg"), "https://a.example/b/isbn/x.jpg");
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
        assert!(cover.bytes.len() > MIN_BYTES);
        assert!(cover.url.starts_with(OPEN_LIBRARY_COVERS));
    }
}
