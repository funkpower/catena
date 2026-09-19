//! The tool against a server, without needing one.
//!
//! The server here is twenty lines of `TcpListener` that answers whatever the
//! test wants it to. That is enough to check the things that actually go
//! wrong: the request shape, the batching, the retry, and what happens when
//! the answer is not what was asked for.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output};
use std::sync::mpsc;
use std::thread;

/// What a request asked for, as the test wants to see it.
struct Asked {
    model: String,
    inputs: Vec<String>,
}

/// Answer `replies` in turn, one per request, and report what was asked.
fn server(replies: Vec<(u16, String)>) -> (String, mpsc::Receiver<Asked>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let host = format!("http://{}", listener.local_addr().expect("an address"));
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        for (at, connection) in listener.incoming().enumerate() {
            let Ok(mut connection) = connection else {
                break;
            };
            if let Some(asked) = serve(&mut connection, replies.get(at)) {
                let _ = sender.send(asked);
            }
            if at + 1 >= replies.len() {
                break;
            }
        }
    });

    (host, receiver)
}

fn serve(connection: &mut TcpStream, reply: Option<&(u16, String)>) -> Option<Asked> {
    let mut reader = BufReader::new(connection.try_clone().expect("a clone"));
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    let request: serde_json::Value = serde_json::from_slice(&body).ok()?;

    let (status, reply) = reply.cloned().unwrap_or((500, String::new()));
    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = connection.write_all(response.as_bytes());
    let _ = connection.flush();

    Some(Asked {
        model: request["model"].as_str().unwrap_or_default().to_string(),
        inputs: request["input"]
            .as_array()
            .map(|inputs| {
                inputs
                    .iter()
                    .map(|input| input.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn vectors(count: usize, width: usize) -> String {
    let one = format!("[{}]", vec!["0.5"; width].join(","));
    format!("{{\"embeddings\":[{}]}}", vec![one; count].join(","))
}

fn run(args: &[&str], stdin: &str) -> Output {
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_emb"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary should run");
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(stdin.as_bytes())
        .expect("the input should write");
    child.wait_with_output().expect("the binary should finish")
}

#[test]
fn every_line_comes_back_as_a_vector() {
    let (host, asked) = server(vec![(200, vectors(2, 4))]);
    let out = run(
        &["--host", &host, "--model", "a-model"],
        "the mill stood\nat the northern end\n",
    );

    assert_eq!(
        out.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<&str> = std::str::from_utf8(&out.stdout)
        .expect("utf-8")
        .lines()
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "[0.5,0.5,0.5,0.5]");

    let asked = asked.recv().expect("a request");
    assert_eq!(asked.model, "a-model");
    assert_eq!(asked.inputs, ["the mill stood", "at the northern end"]);
}

#[test]
fn the_lines_are_sent_in_batches() {
    let (host, asked) = server(vec![(200, vectors(2, 2)), (200, vectors(1, 2))]);
    let out = run(
        &["--host", &host, "--model", "m", "--batch", "2"],
        "one\ntwo\nthree\n",
    );

    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout.lines().count(), 3);
    assert_eq!(asked.recv().expect("a request").inputs.len(), 2);
    assert_eq!(asked.recv().expect("a request").inputs.len(), 1);
}

#[test]
fn a_vector_of_the_wrong_width_is_refused() {
    let (host, _asked) = server(vec![(200, vectors(1, 768))]);
    let out = run(&["--host", &host, "--model", "m", "--dim", "1024"], "one\n");

    assert_eq!(out.status.code(), Some(65), "bad data must exit 65");
    assert!(out.stdout.is_empty(), "a bad answer must write nothing");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("768"), "{said}");
}

#[test]
fn a_server_that_is_not_there_is_reported_as_unavailable() {
    // Port 1 on the loopback: nothing is listening and nothing will be.
    let out = run(
        &[
            "--host",
            "http://127.0.0.1:1",
            "--model",
            "m",
            "--attempts",
            "1",
        ],
        "one\n",
    );
    assert_eq!(out.status.code(), Some(69), "unreachable must exit 69");
}

#[test]
fn a_failed_request_is_tried_again() {
    let (host, asked) = server(vec![(503, "{}".into()), (200, vectors(1, 2))]);
    let out = run(
        &["--host", &host, "--model", "m", "--attempts", "2"],
        "one\n",
    );

    assert_eq!(
        out.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout.lines().count(), 1);
    assert_eq!(asked.recv().expect("a first request").inputs, ["one"]);
    assert_eq!(asked.recv().expect("a second request").inputs, ["one"]);
}

#[test]
fn a_long_line_is_cut_before_it_is_sent() {
    let (host, asked) = server(vec![(200, vectors(1, 2))]);
    let long = "word ".repeat(100);
    run(
        &["--host", &host, "--model", "m", "--max-input-tokens", "5"],
        &format!("{long}\n"),
    );

    let asked = asked.recv().expect("a request");
    assert_eq!(textkit::count(&asked.inputs[0]), 5);
}

#[test]
fn no_input_asks_the_server_nothing() {
    let out = run(&["--host", "http://127.0.0.1:1", "--model", "m"], "");
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}
