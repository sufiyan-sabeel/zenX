# Installing on Linux

Supported architectures: **x86_64** and **aarch64** (ARM64). Both are published
as prebuilt archives and both build from source.

## Installer

```bash
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh
```

This installs to `~/.local/bin/zenx`. The script needs `curl`, `tar` and either
`sha256sum` or `shasum`; all three are present on any normal distribution.

Add the directory to `PATH` if it is not already there:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

Or rerun the installer with `--set-path`.

## Manual install

```bash
arch="$(uname -m)"
case "$arch" in
  x86_64|amd64) target="x86_64-unknown-linux-gnu" ;;
  aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
  *) echo "unsupported architecture: $arch"; exit 1 ;;
esac

curl -fsSLO "https://github.com/sufiyan-sabeel/zenX/releases/latest/download/zenx-${target}.tar.gz"
curl -fsSLO "https://github.com/sufiyan-sabeel/zenX/releases/latest/download/SHA256SUMS.txt"
sha256sum -c SHA256SUMS.txt --ignore-missing

tar -xzf "zenx-${target}.tar.gz"
install -m 755 "zenx-${target}/zenx" ~/.local/bin/zenx
zenx --version
```

## From source

Requirements: a C toolchain and Rust 1.75 or newer. On Debian and Ubuntu:

```bash
sudo apt-get install -y build-essential curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Fedora, `sudo dnf install gcc make curl`; on Arch, `sudo pacman -S base-devel curl`.

```bash
git clone https://github.com/sufiyan-sabeel/zenX
cd zenX
cargo build --release
install -m 755 target/release/zenx ~/.local/bin/zenx
```

`clang` also works in place of `gcc`: SQLite is compiled by `cc`, which accepts
either.

## Uninstall

```bash
zenx uninstall --yes                 # binary only
zenx uninstall --yes --purge-data    # binary and ~/.zenx
```

## Notes

- No system package, service or `/etc` change is made.
- The binary is statically linked against SQLite, so there is no `libsqlite3`
  dependency to satisfy.
- If `~/.local/bin` does not exist, the installer creates it.
