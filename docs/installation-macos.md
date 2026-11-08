# Installing on macOS

Supported architectures: **Apple Silicon (aarch64)** and **Intel (x86_64)**.
Both are published as prebuilt archives.

## Installer

```bash
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh
```

Installs to `~/.local/bin/zenx`. The script uses `curl`, `tar` and `shasum`, all
of which ship with macOS.

Add to `PATH` if needed — the default macOS shell is zsh:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

## Manual install

```bash
target="$(uname -m | sed 's/^arm64$/aarch64/')-apple-darwin"

curl -fsSLO "https://github.com/sufiyan-sabeel/zenX/releases/latest/download/zenx-${target}.tar.gz"
curl -fsSLO "https://github.com/sufiyan-sabeel/zenX/releases/latest/download/SHA256SUMS.txt"
shasum -a 256 -c SHA256SUMS.txt --ignore-missing

tar -xzf "zenx-${target}.tar.gz"
install -m 755 "zenx-${target}/zenx" ~/.local/bin/zenx
```

## Gatekeeper

The release binaries are not signed or notarised yet. If macOS quarantines a
downloaded archive, either install through the script (which uses `curl` and
therefore does not set the quarantine attribute) or clear it explicitly:

```bash
xattr -d com.apple.quarantine ~/.local/bin/zenx
```

## From source

```bash
xcode-select --install     # provides clang, needed to compile SQLite
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/sufiyan-sabeel/zenX
cd zenX
cargo build --release
install -m 755 target/release/zenx ~/.local/bin/zenx
```

## Uninstall

```bash
zenx uninstall --yes
```

Homebrew is not supported yet: there is no formula, and this document will say so
until one exists.
