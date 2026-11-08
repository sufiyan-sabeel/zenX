#!/usr/bin/env bash
# Fail if documentation mentions a zenx subcommand the binary does not have.
#
# The check is deliberately simple: it extracts `zenx <word>` occurrences from
# README.md and docs/*.md, then asks the binary whether that subcommand exists.
set -euo pipefail
cd "$(dirname "$0")/.."

BIN="${ZENX_BIN:-./target/release/zenx}"
if [ ! -x "$BIN" ]; then
  echo "error: $BIN not found; run 'cargo build --release' first" >&2
  exit 1
fi

COMMANDS="$("$BIN" --help | sed -n '/^Commands:/,/^$/p' | awk 'NR>1 && NF {print $1}')"
STATUS=0

while IFS= read -r subcommand; do
  if ! printf '%s\n' "$COMMANDS" | grep -qx "$subcommand"; then
    echo "error: documentation references 'zenx $subcommand', which the binary does not provide" >&2
    STATUS=1
  fi
done < <(grep -rhoE 'zenx [a-z][a-z-]+' README.md docs/*.md 2>/dev/null \
          | awk '{print $2}' | sort -u)

if [ "$STATUS" -eq 0 ]; then
  echo "every documented zenx subcommand exists in the binary"
fi
exit "$STATUS"
