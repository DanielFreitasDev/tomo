#!/usr/bin/env bash
# Full local quality gate — mirrors what CI runs for the Rust side.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."

echo "== cargo fmt =="
cargo fmt --check
echo "== clippy =="
cargo clippy -p tomo-core --all-targets -- -D warnings
echo "== cargo test =="
cargo test --quiet
echo "== ALL RUST CHECKS PASSED =="
