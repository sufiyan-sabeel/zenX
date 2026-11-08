#!/usr/bin/env bash
# Run everything CI runs, locally, in the same order.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== rustfmt"
cargo fmt --all -- --check

echo "== clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "== tests"
cargo test --workspace --all-targets

echo "== documented commands"
./scripts/check-docs-commands.sh

echo "all checks passed"
