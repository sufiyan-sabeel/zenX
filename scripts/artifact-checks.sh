#!/usr/bin/env bash
# Exercise an installed zenX binary the way a user would.
#
# Usage: artifact-checks.sh <path-to-zenx> <dir-with-release-archive>
#
# Every check prints PASS or FAIL; the script exits non-zero if any failed.
# Nothing here publishes or tags anything.
set -uo pipefail

BIN="${1:?usage: artifact-checks.sh <zenx> <dist-dir>}"
DIST="${2:?usage: artifact-checks.sh <zenx> <dist-dir>}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

FAILED=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAILED=$((FAILED + 1)); }

# A fake "newer release" made from the same archive, so update logic can be
# exercised without a real tag.
mkdir -p "$WORK/release"
cp "$DIST"/*.tar.gz "$DIST"/SHA256SUMS.txt "$WORK/release/" 2>/dev/null || true
RELEASE="$WORK/release"

export ZENX_HOME="$WORK/home"
"$BIN" init >/dev/null 2>&1

if version="$("$BIN" --version 2>&1)" && printf '%s' "$version" | grep -q '^zenx [0-9]'; then
  pass "--version ($version)"
else
  fail "--version returned: ${version:-<nothing>}"
fi

help="$("$BIN" --help 2>&1)"
missing=""
for command in init status trace skills doctor mobile bench contract docs update uninstall; do
  printf '%s' "$help" | grep -q " $command" || missing="$missing $command"
done
if [ -z "$missing" ]; then
  pass "--help lists every command"
else
  fail "--help is missing:$missing"
fi

doctor="$("$BIN" doctor . --output json 2>&1)"
if printf '%s' "$doctor" | grep -q '"checks"'; then
  pass "doctor emitted a JSON report"
else
  fail "doctor did not emit JSON"
fi

if [ -d "$RELEASE" ] && [ -f "$RELEASE/SHA256SUMS.txt" ]; then
  check="$("$BIN" update --check --tag v9.9.9 --output json 2>&1)"
  if printf '%s' "$check" | grep -q '"action": "available"'; then
    pass "update --check reports an available release"
  else
    fail "update --check: $check"
  fi

  before="$(sha256_of "$BIN")"
  dry="$(ZENX_RELEASE_BASE_URL="file://$RELEASE" "$BIN" update --dry-run --tag v9.9.9 --output json 2>&1)"
  after="$(sha256_of "$BIN")"
  if printf '%s' "$dry" | grep -q '"verified": true' && [ "$before" = "$after" ]; then
    pass "update --dry-run verified the archive and changed nothing"
  else
    fail "update --dry-run: $dry"
  fi

  install="$(ZENX_RELEASE_BASE_URL="file://$RELEASE" "$BIN" update --tag v9.9.9 --output json 2>&1)"
  if printf '%s' "$install" | grep -q '"action": "installed"' && [ -f "${BIN}.old" ]; then
    pass "update installed and kept a backup"
  else
    fail "update: $install"
  fi

  if "$BIN" update --rollback >/dev/null 2>&1 && [ ! -f "${BIN}.old" ]; then
    pass "update --rollback restored the previous binary"
  else
    fail "update --rollback"
  fi
else
  fail "no release directory to test update against"
fi

plan="$("$BIN" uninstall --output json 2>&1)"
if [ -f "$BIN" ] && [ -f "$ZENX_HOME/zenx.db" ] && printf '%s' "$plan" | grep -q '"binary"'; then
  pass "uninstall only reported its plan"
else
  fail "uninstall plan: $plan"
fi

"$BIN" uninstall --yes >/dev/null 2>&1
if [ ! -f "$BIN" ] && [ -f "$ZENX_HOME/zenx.db" ]; then
  pass "uninstall removed the binary and kept user data"
else
  fail "uninstall removed the wrong things"
fi

printf '\n%d check(s) failed\n' "$FAILED"
[ "$FAILED" -eq 0 ]
