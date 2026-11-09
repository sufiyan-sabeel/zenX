#!/bin/sh
# zenX installer.
#
#   curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh
#
# Downloads the prebuilt archive for this platform, verifies it against the
# release's SHA256SUMS.txt, and only then installs it. If checksums are missing
# or do not match, nothing is extracted and nothing is executed.
#
# Options:
#   --version <tag>   install a specific release (e.g. v0.1.0)
#   --dir <path>      install into this directory
#   --set-path        append the install directory to your shell profile
#   --rollback        restore the binary saved as zenx.old by the last install
#   -h, --help        show this help
#
# Environment:
#   ZENX_RELEASE_BASE_URL  download root override (used by the test suite)
set -eu

REPO="sufiyan-sabeel/zenX"
VERSION=""
INSTALL_DIR=""
SET_PATH=0
ROLLBACK=0

usage() {
  cat <<'USAGE'
zenX installer

  curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh

Downloads the prebuilt archive for this platform, verifies it against the
release's SHA256SUMS.txt, and only then installs it.

Options:
  --version <tag>   install a specific release (e.g. v0.1.0)
  --dir <path>      install into this directory
  --set-path        append the install directory to your shell profile
  --rollback        restore the binary saved as zenx.old by the last install
  -h, --help        show this help

Environment:
  ZENX_RELEASE_BASE_URL  download root override (used by the test suite)
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="${2:?--version needs a value}"; shift 2 ;;
    --version=*) VERSION="${1#*=}"; shift ;;
    --dir) INSTALL_DIR="${2:?--dir needs a value}"; shift 2 ;;
    --dir=*) INSTALL_DIR="${1#*=}"; shift ;;
    --set-path) SET_PATH=1; shift ;;
    --rollback) ROLLBACK=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'zenx installer: unknown option %s\n' "$1" >&2; exit 2 ;;
  esac
done

os="$(uname -s)"
arch="$(uname -m)"
termux=0
if [ -n "${TERMUX_VERSION:-}" ]; then termux=1; fi
if [ -n "${PREFIX:-}" ]; then
  case "$PREFIX" in *com.termux*) termux=1 ;; esac
fi

case "$arch" in
  x86_64|amd64) target_arch="x86_64" ;;
  aarch64|arm64) target_arch="aarch64" ;;
  *)
    printf 'zenx installer: unsupported architecture: %s\n' "$arch" >&2
    printf 'Build from source instead: cargo install --git https://github.com/%s zenx\n' "$REPO" >&2
    exit 1
    ;;
esac

if [ "$termux" -eq 1 ]; then
  # Termux is Android: it uses Bionic, not glibc, so a *-unknown-linux-gnu
  # binary cannot even be loaded. Termux gets its own published artifact.
  case "$target_arch" in
    aarch64) target_os="linux-android" ;;
    *)
      printf 'zenx installer: no prebuilt Termux build for %s (only aarch64 is published)\n' "$arch" >&2
      printf 'Build from source instead: cargo install --git https://github.com/%s zenx\n' "$REPO" >&2
      exit 1
      ;;
  esac
else
  case "$os" in
    Linux) target_os="unknown-linux-gnu" ;;
    Darwin) target_os="apple-darwin" ;;
    *)
      printf 'zenx installer: unsupported operating system: %s\n' "$os" >&2
      printf 'Windows users: run install.ps1 (see docs/installation-windows.md)\n' >&2
      exit 1
      ;;
  esac
fi

target="${target_arch}-${target_os}"
name="zenx-${target}"

if [ -n "${ZENX_RELEASE_BASE_URL:-}" ]; then
  base="${ZENX_RELEASE_BASE_URL%/}"
elif [ -n "$VERSION" ]; then
  base="https://github.com/${REPO}/releases/download/${VERSION}"
else
  base="https://github.com/${REPO}/releases/latest/download"
fi

if [ -z "$INSTALL_DIR" ]; then
  if [ "$termux" -eq 1 ] && [ -n "${PREFIX:-}" ] && [ -d "${PREFIX}/bin" ]; then
    INSTALL_DIR="${PREFIX}/bin"
  else
    INSTALL_DIR="${HOME}/.local/bin"
  fi
fi

if ! mkdir -p "$INSTALL_DIR" 2>/dev/null; then
  printf 'zenx installer: cannot create %s\n' "$INSTALL_DIR" >&2
  exit 1
fi
if [ ! -w "$INSTALL_DIR" ]; then
  printf 'zenx installer: no write permission for %s\n' "$INSTALL_DIR" >&2
  printf 'Choose another directory with --dir <writable-dir>\n' >&2
  exit 1
fi

if [ "$ROLLBACK" -eq 1 ]; then
  dest="${INSTALL_DIR}/zenx"
  if [ ! -f "${dest}.old" ]; then
    printf 'zenx installer: no previous binary at %s.old, nothing to roll back\n' "$dest" >&2
    exit 1
  fi
  mv "${dest}.old" "$dest"
  chmod 755 "$dest" 2>/dev/null || true
  rolled="$("$dest" --version 2>/dev/null || true)"
  printf 'zenx installer: rolled back to %s at %s\n' "${rolled:-the previous binary}" "$dest"
  exit 0
fi

existing=""
if command -v zenx >/dev/null 2>&1; then
  existing="$(command -v zenx)"
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/zenx-install.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT INT TERM HUP

proto="--proto =https --tlsv1.2"
case "$base" in
  file://*) proto="" ;;
  https://*) ;;
  *) printf 'zenx installer: refusing to download from %s (only https is allowed)\n' "$base" >&2; exit 1 ;;
esac

printf 'zenx installer: platform %s\n' "$target"
if [ -n "$existing" ]; then
  printf 'zenx installer: replacing the existing installation at %s\n' "$existing"
fi
printf 'zenx installer: downloading %s.tar.gz\n' "$name"

# shellcheck disable=SC2086
if ! curl -fsSL $proto "$base/${name}.tar.gz" -o "$tmp/${name}.tar.gz"; then
  printf 'zenx installer: download failed for %s/%s.tar.gz\n' "$base" "$name" >&2
  printf 'Check that a release exists: https://github.com/%s/releases\n' "$REPO" >&2
  exit 1
fi

# shellcheck disable=SC2086
if ! curl -fsSL $proto "$base/SHA256SUMS.txt" -o "$tmp/SHA256SUMS.txt"; then
  printf 'zenx installer: refusing to install: this release publishes no SHA256SUMS.txt\n' >&2
  printf 'An unverifiable download is never installed.\n' >&2
  exit 1
fi

expected="$(awk -v want="${name}.tar.gz" '$2 == want || $2 == "*" want { print $1 }' "$tmp/SHA256SUMS.txt" | head -n 1)"
if [ -z "$expected" ]; then
  printf 'zenx installer: refusing to install: SHA256SUMS.txt has no entry for %s.tar.gz\n' "$name" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/${name}.tar.gz" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$tmp/${name}.tar.gz" | awk '{print $1}')"
else
  printf 'zenx installer: no SHA-256 tool found (need sha256sum or shasum)\n' >&2
  exit 1
fi

if [ "$(printf '%s' "$actual" | tr '[:upper:]' '[:lower:]')" != "$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')" ]; then
  printf 'zenx installer: checksum mismatch, refusing to install\n' >&2
  printf '  published: %s\n  downloaded: %s\n' "$expected" "$actual" >&2
  exit 1
fi
printf 'zenx installer: checksum verified (%s)\n' "$actual"

tar -xzf "$tmp/${name}.tar.gz" -C "$tmp"
src="$tmp/${name}/zenx"
if [ ! -f "$src" ]; then
  printf 'zenx installer: the archive did not contain a zenx binary\n' >&2
  exit 1
fi

dest="$INSTALL_DIR/zenx"
if [ -f "$dest" ]; then
  cp -p "$dest" "${dest}.old" 2>/dev/null || true
fi
if ! install -m 755 "$src" "$dest" 2>/dev/null; then
  cp "$src" "$dest" && chmod 755 "$dest"
fi

installed_version="$("$dest" --version 2>/dev/null || true)"
if [ -z "$installed_version" ]; then
  printf 'zenx installer: installed binary did not run; leaving %s in place\n' "${dest}.old" >&2
  exit 1
fi

printf 'zenx installer: installed %s to %s\n' "$installed_version" "$dest"

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*)
    ;;
  *)
    if [ "$SET_PATH" -eq 1 ]; then
      profile="${HOME}/.profile"
      case "${SHELL:-}" in
        */zsh) profile="${HOME}/.zshrc" ;;
        */bash) profile="${HOME}/.bashrc" ;;
      esac
      line="export PATH=\"${INSTALL_DIR}:\$PATH\""
      if grep -qF "$line" "$profile" 2>/dev/null; then
        printf 'zenx installer: %s is already configured in %s\n' "$INSTALL_DIR" "$profile"
      else
        printf '\n# added by the zenX installer\n%s\n' "$line" >> "$profile"
        printf 'zenx installer: added %s to PATH in %s\n' "$INSTALL_DIR" "$profile"
        printf 'Open a new shell, or run: . %s\n' "$profile"
      fi
    else
      printf 'zenx installer: %s is not on PATH yet\n' "$INSTALL_DIR"
      # The literal $PATH is intentional: this line is meant to be copied into a shell.
      # shellcheck disable=SC2016
      printf '  export PATH="%s:$PATH"\n' "$INSTALL_DIR"
      printf '  or rerun with --set-path to write that line for you\n'
    fi
    ;;
esac

printf '\nNext: zenx init && zenx status\n'
