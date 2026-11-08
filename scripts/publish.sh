#!/usr/bin/env bash
# Publish every crate to crates.io, in dependency order.
#
# Nothing here has been run against crates.io yet: publishing requires an API
# token and is irreversible (a version number can never be reused). Run --plan
# first, then --dry-run, and only then --yes.
#
# Usage:
#   scripts/publish.sh --plan      # print the exact commands, change nothing
#   scripts/publish.sh --dry-run   # package and verify every crate locally
#   scripts/publish.sh --yes       # publish for real
#
# Note on --dry-run: crates.io must already have the earlier crates for a
# dependent's dry-run to resolve its dependencies, so a full dry-run of the
# chain before the first publish can fail for every crate after zenx-core.
# That is a property of publishing to a registry, not a packaging defect.
set -euo pipefail
cd "$(dirname "$0")/.."

MODE="${1:---plan}"

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

VERSION="$(python3 -c 'import tomllib,pathlib;print(tomllib.loads(pathlib.Path("Cargo.toml").read_text())["workspace"]["package"]["version"])' 2>/dev/null || grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"

case "$MODE" in
  --plan)
    echo "publish plan for zenX $VERSION"
    echo
    position=0
    for package in "${ORDER[@]}"; do
      position=$((position + 1))
      printf '%2d. cargo package --no-verify --allow-dirty --package %s\n' "$position" "$package"
      printf '    cargo publish --dry-run --package %s\n' "$package"
      printf '    cargo publish --package %s\n' "$package"
    done
    echo
    echo "then verify: cargo install zenx --version $VERSION"
    exit 0
    ;;
  --dry-run|--yes) ;;
  *)
    echo "usage: $0 [--plan|--dry-run|--yes]" >&2
    exit 2
    ;;
esac

if [ "$MODE" = "--yes" ] && [ -z "${CARGO_REGISTRY_TOKEN:-}" ]; then
  echo "error: set CARGO_REGISTRY_TOKEN before publishing" >&2
  exit 1
fi

for package in "${ORDER[@]}"; do
  echo "== $package"
  if [ "$MODE" = "--dry-run" ]; then
    cargo package --no-verify --allow-dirty --package "$package"
    cargo publish --dry-run --package "$package"
  else
    cargo publish --package "$package"
    # The index needs a moment before dependents can resolve the new version.
    sleep 30
  fi
done

echo
echo "done for: ${ORDER[*]}"
echo "verify: cargo install zenx --version $VERSION"
