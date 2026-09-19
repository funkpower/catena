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
    let jobs: [Job; 16] = [
        ("tokens", tokens),
        ("unescape", unescape),
        ("chunkmd", chunkmd),
        ("mdscrub", mdscrub),
        ("pdftriage", pdftriage),
        ("cuenorm", cuenorm),
        ("cueseg", cueseg),
        ("rrf", rrf),
        ("cuepack", cuepack),
        ("booksniff", booksniff),
        ("bibclean", bibclean),
        ("slug", slug),
        ("author", author),
        ("vocab", vocab),
        ("mdsplit", mdsplit),
        ("avplan", avplan),
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

fn mdsplit() -> Written {
    let dir = parity::golden("mdsplit");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut written = Vec::new();
    for case in parity::cases("mdsplit") {
        let markdown = std::fs::read_to_string(&case).map_err(|e| e.to_string())?;
        let sections = parity::run_oracle("mdsplit.py", &markdown)?;
        let path = dir.join(format!("{}.json", parity::case_name(&case)));
        std::fs::write(&path, sections).map_err(|e| e.to_string())?;
        written.push(path.display().to_string());
    }
    Ok(written)
}

fn vocab() -> Written {
    let dir = parity::golden("vocab");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let cases = parity::dir().join("cases/vocab/lists.ndjson");
    let cases = std::fs::read_to_string(&cases).map_err(|e| e.to_string())?;
    let judgements = parity::run_oracle("vocab.py", &cases)?;
    let path = dir.join("lists.ndjson");
    std::fs::write(&path, judgements).map_err(|e| e.to_string())?;
    Ok(vec![path.display().to_string()])
}

fn author() -> Written {
    let dir = parity::golden("author");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let cases = parity::dir().join("cases/author/records.ndjson");
    let cases = std::fs::read_to_string(&cases).map_err(|e| e.to_string())?;
    let judgements = parity::run_oracle("author.py", &cases)?;
    let path = dir.join("records.ndjson");
    std::fs::write(&path, judgements).map_err(|e| e.to_string())?;
    Ok(vec![path.display().to_string()])
}

fn slug() -> Written {
    let dir = parity::golden("slug");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let cases = parity::dir().join("cases/slug/records.ndjson");
    let cases = std::fs::read_to_string(&cases).map_err(|e| e.to_string())?;
    let judgements = parity::run_oracle("slug.py", &cases)?;
    let path = dir.join("records.ndjson");
    std::fs::write(&path, judgements).map_err(|e| e.to_string())?;
    Ok(vec![path.display().to_string()])
}

fn bibclean() -> Written {
    let dir = parity::golden("bibclean");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let cases = parity::dir().join("cases/bibclean/records.ndjson");
    let cases = std::fs::read_to_string(&cases).map_err(|e| e.to_string())?;
    let judgements = parity::run_oracle("bibclean.py", &cases)?;
    let path = dir.join("records.ndjson");
    std::fs::write(&path, judgements).map_err(|e| e.to_string())?;
    Ok(vec![path.display().to_string()])
}

fn booksniff() -> Written {
    let dir = parity::golden("booksniff");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let cases = parity::dir().join("cases/booksniff/files.ndjson");
    let cases = std::fs::read_to_string(&cases).map_err(|e| e.to_string())?;
    let verdicts = parity::run_oracle("booksniff.py", &cases)?;
    let path = dir.join("files.ndjson");
    std::fs::write(&path, verdicts).map_err(|e| e.to_string())?;
    Ok(vec![path.display().to_string()])
}

fn cuepack() -> Written {
    let dir = parity::golden("cuepack");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let budgets = [("default", 75.0, 512), ("small", 12.0, 24)];
    let mut written = Vec::new();
    for case in parity::cases_with_extension("cuepack", "ndjson") {
        let name = parity::case_name(&case);
        let cues = std::fs::read_to_string(&case).map_err(|e| e.to_string())?;
        for (set, seconds, max_tokens) in budgets {
            let packed = parity::run_oracle_with(
                "cuepack.py",
                &[seconds.to_string(), max_tokens.to_string()],
                &cues,
            )?;
            let path = dir.join(format!("{name}.{set}.ndjson"));
            std::fs::write(&path, packed).map_err(|e| e.to_string())?;
            written.push(path.display().to_string());
        }
    }
    Ok(written)
}

fn rrf() -> Written {
    let dir = parity::golden("rrf");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut written = Vec::new();
    for case in parity::cases_with_extension("rrf", "ndjson") {
        let hits = std::fs::read_to_string(&case).map_err(|e| e.to_string())?;
        let fused = parity::run_oracle("rrf.py", &hits)?;
        let path = dir.join(format!("{}.ndjson", parity::case_name(&case)));
        std::fs::write(&path, fused).map_err(|e| e.to_string())?;
        written.push(path.display().to_string());
    }
    Ok(written)
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

fn avplan() -> Written {
    let dir = parity::golden("avplan");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let cases = parity::dir().join("cases/avplan/cases.ndjson");
    let text = std::fs::read_to_string(&cases).map_err(|e| e.to_string())?;
    let verdicts = parity::run_node_oracle("compatible.mjs", &text)?;
    let path = dir.join("cases.ndjson");
    std::fs::write(&path, verdicts).map_err(|e| e.to_string())?;
    Ok(vec![path.display().to_string()])
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
