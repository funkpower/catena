//! Differential tests against the Python implementations these tools were
//! ported from.
//!
//! The risk in this port is not that Rust is hard. It is that the heuristics
//! were tuned against a real corpus over a long time, and a reimplementation
//! that is 98% right is worse than no port at all, because the 2% is silent.
//!
//! So every ported function is pinned to what the Python produced, by golden
//! files under `parity/golden/`. The default test run compares Rust against
//! those files and needs nothing but Rust. With `--features python` the
//! oracle is re-run live, which catches the golden files drifting away from
//! a Python implementation that has moved on.
//!
//! Fixtures are generated, synthetic, or public domain. No corpus text is
//! committed here, and for copyrighted sources only the derived signal — the
//! scores, the counts, the timings — is ever checked in.

pub mod budgets;
pub mod sample;

use std::path::{Path, PathBuf};

/// The `parity/` directory, wherever this crate was checked out to.
pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Path to a golden file.
pub fn golden(name: &str) -> PathBuf {
    dir().join("golden").join(name)
}

/// Path to an oracle script.
pub fn oracle(name: &str) -> PathBuf {
    dir().join("oracle").join(name)
}

/// Cases where `cuenorm` is meant to differ from the JavaScript it was
/// ported from, each one argued for in `KNOWN.md`.
pub const DIVERGENT_CUENORM: &[&str] = &["coordinates"];

/// The JavaScript runtime to run an oracle with.
pub fn node() -> String {
    std::env::var("CATENA_PARITY_NODE").unwrap_or_else(|_| "node".to_string())
}

/// Run a JavaScript oracle over `input` and return its stdout.
pub fn run_node_oracle(script: &str, input: &str) -> Result<String, String> {
    run_oracle_in(node(), script, &[], input)
}

/// The Python interpreter to run an oracle with.
///
/// Taken from `CATENA_PARITY_PYTHON`, because the interpreter that has the
/// pipeline's dependencies installed is a fact about one machine and must not
/// be compiled in.
pub fn python() -> String {
    std::env::var("CATENA_PARITY_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// Run an oracle script over `input` and return its stdout.
///
/// The interpreter comes from the environment and the script from this
/// directory, so nothing about where a Python pipeline lives on a particular
/// machine is compiled in.
pub fn run_oracle(script: &str, input: &str) -> Result<String, String> {
    run_oracle_with(script, &[], input)
}

/// Run an oracle script with arguments.
pub fn run_oracle_with(script: &str, args: &[String], input: &str) -> Result<String, String> {
    run_oracle_in(python(), script, args, input)
}

/// Run an oracle under a named runtime.
pub fn run_oracle_in(
    runtime: String,
    script: &str,
    args: &[String],
    input: &str,
) -> Result<String, String> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let script = oracle(script);
    let mut child = Command::new(&runtime)
        .arg(&script)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("running {} with {runtime}: {e}", script.display()))?;
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(input.as_bytes())
        .map_err(|e| format!("writing to {}: {e}", script.display()))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("waiting for {}: {e}", script.display()))?;
    if !out.status.success() {
        return Err(format!("{} exited with {}", script.display(), out.status));
    }
    String::from_utf8(out.stdout)
        .map_err(|e| format!("{} wrote invalid UTF-8: {e}", script.display()))
}

/// Every case file for a tool, in a fixed order.
pub fn cases(tool: &str) -> Vec<PathBuf> {
    cases_with_extension(tool, "md")
}

/// Every case file for a tool with a given extension, in a fixed order.
pub fn cases_with_extension(tool: &str, extension: &str) -> Vec<PathBuf> {
    let dir = dir().join("cases").join(tool);
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    found.sort();
    found
}

/// The stem of a case path, which names its golden file.
pub fn case_name(path: &Path) -> String {
    path.file_stem()
        .expect("a case file has a name")
        .to_string_lossy()
        .into_owned()
}

/// Read a golden file whole.
pub fn read_golden_text(name: &str) -> String {
    let path = golden(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Read a golden file, dropping its `#` header lines.
pub fn read_golden(name: &str) -> Vec<String> {
    let path = golden(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}
