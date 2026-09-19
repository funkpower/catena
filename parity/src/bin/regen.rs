//! Regenerates the golden files by running the Python oracles.
//!
//! Never run as part of a test: a golden file that regenerates itself when
//! it fails is a test that cannot fail. Run it deliberately, read the diff,
//! and commit the change with the reason.
//!
//!     cargo run -p parity --bin regen

use std::process::ExitCode;

fn main() -> ExitCode {
    match regen_tokens() {
        Ok(path) => {
            eprintln!("wrote {path}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("regen: {e}");
            ExitCode::FAILURE
        }
    }
}

fn regen_tokens() -> Result<String, String> {
    let header = format!(
        "# cl100k_base token counts, one per line of parity::sample::standard()\n\
         # seed={:#018x} lines={}\n",
        parity::sample::SEED,
        parity::sample::LINES,
    );
    let counts = parity::run_oracle("tokens.py", &parity::sample::standard())?;
    let path = parity::golden("tokens.counts");
    std::fs::write(&path, format!("{header}{counts}")).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}
