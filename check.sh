#!/bin/sh
# Everything that has to pass before a commit leaves the machine.
#
# The same script runs locally and in CI, so there is one gate and not two
# that drift apart.
set -eu

cd "$(dirname "$0")"

say() { printf '\n== %s\n' "$1"; }

say "formatting"
cargo fmt --check

say "lints"
cargo clippy --workspace --all-targets --all-features -- -D warnings

say "tests"
cargo test --workspace

say "docs"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --quiet

say "documentation links"
if command -v python3 >/dev/null 2>&1; then
	python3 tools/check-links.py
else
	echo "skipped: no python3"
fi

# Differential tests against the Python these tools were ported from. Skipped
# unless an interpreter with the dependencies is named, since most machines do
# not have one.
if [ -n "${CATENA_PARITY_PYTHON:-}" ]; then
	say "parity against python"
	cargo test -p parity --features python
else
	say "parity against python (skipped: set CATENA_PARITY_PYTHON to run it)"
fi

printf '\nall clear\n'
