//! The tool as a caller meets it: files in, records and an exit code out.

use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_booksniff"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn written(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join("booksniff-tests");
    std::fs::create_dir_all(&dir).expect("a writable temp directory");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("a writable path");
    path
}

fn padded(head: &[u8]) -> Vec<u8> {
    let mut out = head.to_vec();
    out.resize(8192, b'x');
    out
}

#[test]
fn a_file_it_recognises_comes_back_as_a_record() {
    let path = written("book.pdf", &padded(b"%PDF-1.7\n"));
    let out = run(&[path.to_str().expect("utf-8 path")]);
    assert_eq!(out.status.code(), Some(0));

    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON record");
    assert_eq!(doc["kind"], "pdf");
    assert_eq!(doc["declared_ext"], "pdf");
    assert_eq!(doc["bytes"], 8192);
    assert!(out.stderr.is_empty());
}

#[test]
fn a_file_it_cannot_place_answers_no() {
    let path = written("mystery.bin", &padded(&[0x00, 0x01, 0x02]));
    let out = run(&[path.to_str().expect("utf-8 path")]);
    assert_eq!(out.status.code(), Some(1), "unknown must answer no");
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON record");
    assert_eq!(doc["kind"], "unknown");
}

#[test]
fn several_files_come_back_as_several_records() {
    let one = written("one.pdf", &padded(b"%PDF-1.7"));
    let two = written("two.djvu", &padded(b"AT&TFORM"));
    let out = run(&[
        one.to_str().expect("utf-8 path"),
        two.to_str().expect("utf-8 path"),
    ]);
    let kinds: Vec<String> = String::from_utf8(out.stdout)
        .expect("utf-8")
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("valid JSON")["kind"].to_string()
        })
        .collect();
    assert_eq!(kinds, ["\"pdf\"", "\"djvu\""]);
}

#[test]
fn the_formats_read_from_the_front_come_back_by_their_wire_names() {
    let rtf = written("letter.rtf", &padded(br"{\rtf1\ansi"));
    let lit = written("book.lit", &padded(b"ITOLITLS"));
    let fb2 = written(
        "mill.fb2",
        &padded(b"<?xml version=\"1.0\"?>\n<FictionBook xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\">"),
    );
    let out = run(&[
        rtf.to_str().expect("utf-8 path"),
        lit.to_str().expect("utf-8 path"),
        fb2.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(out.status.code(), Some(0));
    let kinds: Vec<String> = String::from_utf8(out.stdout)
        .expect("utf-8")
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("valid JSON")["kind"].to_string()
        })
        .collect();
    assert_eq!(kinds, ["\"rtf\"", "\"lit\"", "\"fb2\""]);
}

#[test]
fn a_file_that_is_not_there_is_an_io_error_and_the_rest_still_run() {
    let path = written("kept.pdf", &padded(b"%PDF-1.7"));
    let out = run(&[
        "/definitely/not/here.pdf",
        path.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(out.status.code(), Some(74));
    assert!(!out.stderr.is_empty());
    // The readable file was still identified, on stdout, uncontaminated.
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON record");
    assert_eq!(doc["kind"], "pdf");
}

#[test]
fn the_plain_form_is_for_a_person() {
    let path = written("lying.pdf", &padded(b"AT&TFORM"));
    let out = run(&["--plain", path.to_str().expect("utf-8 path")]);
    let said = String::from_utf8(out.stdout).expect("utf-8");
    assert!(said.starts_with("djvu\t"), "{said}");
    assert!(said.contains("named .pdf"), "{said}");
}

#[test]
fn no_files_named_is_a_usage_error() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(64), "usage errors exit 64");
    assert!(out.stdout.is_empty());
}
