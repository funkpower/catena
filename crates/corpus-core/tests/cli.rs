//! The schema command, as a consumer in another language would meet it.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_corpus-core"))
        .args(args)
        .output()
        .expect("the binary should run")
}

#[test]
fn a_named_record_prints_its_schema() {
    let out = run(&["schema", "doc"]);
    assert_eq!(out.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(json["title"], "Doc");
    assert!(out.stderr.is_empty(), "diagnostics leaked to stdout's side");
}

#[test]
fn no_record_named_means_all_of_them() {
    let out = run(&["schema"]);
    assert_eq!(out.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    for record in ["doc", "chunk", "hit"] {
        assert!(json.get(record).is_some(), "missing {record}");
    }
}

#[test]
fn an_unknown_record_is_a_usage_error_with_nothing_on_stdout() {
    let out = run(&["schema", "nope"]);
    assert_eq!(out.status.code(), Some(64));
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());
}

#[test]
fn arguments_that_mean_nothing_are_refused_rather_than_ignored() {
    for args in [
        vec![],
        vec!["schema", "doc", "junk"],
        vec!["nonsense"],
        vec!["--json"],
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(64), "for {args:?}");
    }
}

#[test]
fn it_says_what_version_it_is() {
    let out = run(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    let said = String::from_utf8(out.stdout).unwrap();
    assert!(said.contains(env!("CARGO_PKG_VERSION")), "{said:?}");
}

#[test]
fn the_same_schema_comes_out_byte_identical_every_time() {
    assert_eq!(run(&["schema", "all"]).stdout, run(&["schema", "all"]).stdout);
}
