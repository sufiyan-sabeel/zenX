#!/usr/bin/env bash
# Publish every crate to crates.io, in dependency order.
#
# Nothing here has been run by the project yet: publishing requires an API token
# and is irreversible (a version number can never be reused). Run with
# --dry-run first and read the output.
#
# Usage:
#   scripts/publish.sh --dry-run     # package and verify, publish nothing
#   scripts/publish.sh --yes         # publish for real
set -euo pipefail
cd "$(dirname "$0")/.."

MODE="${1:---dry-run}"
DRY=0
case "$MODE" in
  --dry-run) DRY=1 ;;
  --yes) DRY=0 ;;
  *) echo "usage: $0 [--dry-run|--yes]" >&2; exit 2 ;;
esac

# Dependencies must be on crates.io before their dependents.
ORDER=(
  zenx-core
  zenx-security
  zenx-storage
  zenx-trace
  zenx-skills
  zenx-doctor
  zenx-mobile
  zenx-bench
  zenx-contract
  zenx-docs
  zenx-selfupdate
  zenx
)

if [ -z "${CARGO_REGISTRY_TOKEN:-}" ] && [ "$DRY" -eq 0 ]; then
  echo "error: set CARGO_REGISTRY_TOKEN before publishing" >&2
  exit 1
fi

for package in "${ORDER[@]}"; do
  echo "== $package"
  if [ "$DRY" -eq 1 ]; then
    cargo package --package "$package" --allow-dirty
    cargo publish --package "$package" --dry-run
  else
    cargo publish --package "$package"
    # The index needs a moment before dependents can resolve the new version.
    sleep 30
  fi
done

echo
echo "published in this order: ${ORDER[*]}"
echo "verify: cargo install zenx --version $(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
