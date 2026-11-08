# Installing zenX

## Prebuilt binary

Linux (x86_64, aarch64), macOS (Intel, Apple Silicon) and Windows archives are
attached to every GitHub release, with `SHA256SUMS.txt`.

```bash
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/scripts/install.sh | bash
```

The script picks the archive for your OS and architecture, verifies the checksum
when one is published, and installs to `$PREFIX/bin` on Termux or
`~/.local/bin` elsewhere. Override with `ZENX_PREFIX` and `ZENX_VERSION`.

Manual install on Linux or macOS:

```bash
curl -fsSLO https://github.com/sufiyan-sabeel/zenX/releases/latest/download/zenx-x86_64-unknown-linux-gnu.tar.gz
curl -fsSLO https://github.com/sufiyan-sabeel/zenX/releases/latest/download/SHA256SUMS.txt
sha256sum -c SHA256SUMS.txt --ignore-missing
tar -xzf zenx-x86_64-unknown-linux-gnu.tar.gz
install -m 755 zenx-x86_64-unknown-linux-gnu/zenx ~/.local/bin/zenx
```

On Windows, download `zenx-x86_64-pc-windows-msvc.zip`, extract it, and put
`zenx.exe` somewhere on `PATH`.

## From source

Requirements: a stable Rust toolchain (1.75 or newer) and a C compiler, because
SQLite is compiled from source.

```bash
git clone https://github.com/sufiyan-sabeel/zenX
cd zenX
cargo build --release
install -m 755 target/release/zenx ~/.local/bin/zenx
```

The C compiler is needed by `rusqlite`'s bundled SQLite: `gcc` on most Linux
distributions, `clang` from the Xcode command line tools on macOS, MSVC build
tools on Windows, and `pkg install clang` on Termux.

## First run

```bash
zenx init
zenx status
```

`zenx init` creates `~/.zenx` (or `$ZENX_HOME`), writes a default `config.toml`
and creates the database. It never overwrites an existing config unless you pass
`--force`.

## Uninstall

Remove the binary and, if you want the data gone too:

```bash
rm -rf ~/.zenx
```

Nothing else is installed: no system service, no shell profile edits, no global
state outside that directory.
