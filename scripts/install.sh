#!/usr/bin/env bash
# Install a zenX release binary.
#
# Usage: curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/scripts/install.sh | bash
# Environment:
#   ZENX_VERSION  release tag to install (default: latest)
#   ZENX_PREFIX   install directory (default: $PREFIX/bin on Termux, else ~/.local/bin)
set -euo pipefail

REPO="sufiyan-sabeel/zenX"
VERSION="${ZENX_VERSION:-latest}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)  target_os="unknown-linux-gnu" ;;
  Darwin) target_os="apple-darwin" ;;
  *) echo "unsupported OS: $os. Windows users: download the .zip from the releases page." >&2; exit 1 ;;
esac

case "$arch" in
  x86_64|amd64) target_arch="x86_64" ;;
  aarch64|arm64) target_arch="aarch64" ;;
  *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
esac

target="${target_arch}-${target_os}"
name="zenx-${target}"

if [ "$VERSION" = "latest" ]; then
  base="https://github.com/${REPO}/releases/latest/download"
else
  base="https://github.com/${REPO}/releases/download/${VERSION}"
fi

if [ -n "${PREFIX:-}" ] && [ -d "${PREFIX}/bin" ]; then
  install_dir="${ZENX_PREFIX:-${PREFIX}/bin}"
else
  install_dir="${ZENX_PREFIX:-${HOME}/.local/bin}"
fi
mkdir -p "$install_dir"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "downloading ${name}.tar.gz"
curl -fsSL "${base}/${name}.tar.gz" -o "${tmp}/${name}.tar.gz"

echo "downloading checksums"
if curl -fsSL "${base}/SHA256SUMS.txt" -o "${tmp}/SHA256SUMS.txt"; then
  ( cd "$tmp" && grep " ${name}.tar.gz$" SHA256SUMS.txt | sha256sum -c - )
else
  echo "warning: no SHA256SUMS.txt published for this release; skipping verification" >&2
fi

tar -C "$tmp" -xzf "${tmp}/${name}.tar.gz"
install -m 755 "${tmp}/${name}/zenx" "${install_dir}/zenx"

echo "installed $("${install_dir}/zenx" --version) to ${install_dir}/zenx"
case ":$PATH:" in
  *":${install_dir}:"*) ;;
  *) echo "note: add ${install_dir} to PATH" ;;
esac
