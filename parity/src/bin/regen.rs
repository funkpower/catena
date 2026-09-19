//! Regenerates the golden files by running the Python oracles.
//!
//! Never run as part of a test: a golden file that regenerates itself when
//! it fails is a test that cannot fail. Run it deliberately, read the diff,
//! and commit the change with the reason.
//!
//! ```text
//! CATENA_PARITY_PYTHON=... CATENA_PARITY_PYTHONPATH=... \
//!     cargo run -p parity --bin regen
//! ```

use std::process::ExitCode;

/// What one oracle run produces: the golden files it wrote, or why it could
/// not.
type Written = Result<Vec<String>, String>;

/// One oracle and the name it is reported under.
type Job = (&'static str, fn() -> Written);

fn main() -> ExitCode {
    let jobs: [Job; 7] = [
        ("tokens", tokens),
        ("unescape", unescape),
        ("chunkmd", chunkmd),
        ("mdscrub", mdscrub),
        ("pdftriage", pdftriage),
        ("cuenorm", cuenorm),
        ("cueseg", cueseg),
    ];

    for (name, job) in jobs {
        match job() {
            Ok(written) => {
                for path in written {
                    eprintln!("wrote {path}");
                }
            }
            Err(e) => {
                eprintln!("regen {name}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

fn tokens() -> Written {
    let header = format!(
        "# cl100k_base token counts, one per line of parity::sample::standard()\n\
         # seed={:#018x} lines={}\n",
        parity::sample::SEED,
        parity::sample::LINES,
    );
    let counts = parity::run_oracle("tokens.py", &parity::sample::standard())?;
    let path = parity::golden("tokens.counts");
    std::fs::write(&path, format!("{header}{counts}")).map_err(|e| e.to_string())?;
    Ok(vec![path.display().to_string()])
}

fn unescape() -> Written {
    let dir = parity::golden("unescape");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let cases = parity::dir().join("cases/unescape/entities.txt");
    let text = std::fs::read_to_string(&cases).map_err(|e| e.to_string())?;
    let decoded = parity::run_oracle("unescape.py", &text)?;
    let path = dir.join("entities.jsonl");
    std::fs::write(&path, decoded).map_err(|e| e.to_string())?;
    Ok(vec![path.display().to_string()])
}

fn cueseg() -> Written {
    let dir = parity::golden("cueseg");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let table = parity::dir().join("cases/cueseg/durations.json");
    let table = std::fs::read_to_string(&table).map_err(|e| e.to_string())?;
    let durations: std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>> =
        serde_json::from_str(&table).map_err(|e| e.to_string())?;

    let mut written = Vec::new();
    for case in parity::cases_with_extension("cueseg", "ndjson") {
        let name = parity::case_name(&case);
        let Some(spans) = durations.get(&name) else {
            continue;
        };
        let cues = std::fs::read_to_string(&case).map_err(|e| e.to_string())?;
        for (span, duration) in spans {
            let chapters = parity::run_oracle_with("cueseg.py", &[format!("{duration}")], &cues)?;
            let path = dir.join(format!("{name}.{span}.ndjson"));
            std::fs::write(&path, chapters).map_err(|e| e.to_string())?;
            written.push(path.display().to_string());
        }
    }
    Ok(written)
}

fn cuenorm() -> Written {
    let dir = parity::golden("cuenorm");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut written = Vec::new();
    for case in parity::cases_with_extension("cuenorm", "srt") {
        let content = std::fs::read_to_string(&case).map_err(|e| e.to_string())?;
        let cues = parity::run_node_oracle("cuenorm.mjs", &content)?;
        let path = dir.join(format!("{}.ndjson", parity::case_name(&case)));
        std::fs::write(&path, cues).map_err(|e| e.to_string())?;
        written.push(path.display().to_string());
    }
    Ok(written)
}

fn pdftriage() -> Written {
    let dir = parity::golden("pdftriage");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let cases = parity::dir().join("cases/pdftriage/samples.ndjson");
    let text = std::fs::read_to_string(&cases).map_err(|e| e.to_string())?;
    let measured = parity::run_oracle("pdftriage.py", &text)?;
    let path = dir.join("samples.ndjson");
    std::fs::write(&path, measured).map_err(|e| e.to_string())?;
    Ok(vec![path.display().to_string()])
}

fn mdscrub() -> Written {
    let dir = parity::golden("mdscrub");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut written = Vec::new();
    for case in parity::cases("mdscrub") {
        let markdown = std::fs::read_to_string(&case).map_err(|e| e.to_string())?;
        let name = parity::case_name(&case);
        let cleaned = parity::run_oracle("mdscrub.py", &markdown)?;
        let path = dir.join(format!("{name}.md"));
        std::fs::write(&path, cleaned).map_err(|e| e.to_string())?;
        written.push(path.display().to_string());
    }
    Ok(written)
}

fn chunkmd() -> Written {
    let dir = parity::golden("chunkmd");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut written = Vec::new();
    for case in parity::cases("chunkmd") {
        let markdown = std::fs::read_to_string(&case).map_err(|e| e.to_string())?;
        for (set, budgets) in parity::budgets::SETS {
            let records = parity::run_oracle_with(
                "chunkmd.py",
                &parity::budgets::as_args(budgets),
                &markdown,
            )?;
            let path = dir.join(format!("{}.{set}.ndjson", parity::case_name(&case)));
            std::fs::write(&path, records).map_err(|e| e.to_string())?;
            written.push(path.display().to_string());
        }
    }
    Ok(written)
}
