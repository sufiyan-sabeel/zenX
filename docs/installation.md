# Installation

zenX installs two ways: a prebuilt binary through the shell or PowerShell
installer, or a Cargo build from source. Both put a single `zenx` executable on
your machine; nothing else is installed, and no root access is needed.

| Platform | Prebuilt | Cargo from source |
|----------|----------|-------------------|
| Linux x86_64 | yes | yes |
| Linux aarch64 | yes | yes |
| Android / Termux aarch64 | yes | yes |
| macOS x86_64 | yes | yes |
| macOS aarch64 (Apple Silicon) | yes | yes |
| Windows x86_64 | yes | yes |
| Windows ARM64 | not yet | yes |

Platform-specific guides: [Linux](installation-linux.md),
[macOS](installation-macos.md), [Windows](installation-windows.md),
[Android and Termux](installation-termux.md).

## Method 1 — the installer

```bash
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh
```

The script detects the operating system and CPU architecture, downloads the
matching archive from GitHub Releases, and **verifies it against the release's
`SHA256SUMS.txt` before extracting anything**. If the checksums are missing, or
do not match, it stops: an unverifiable download is never unpacked and never
executed.

Options:

```bash
# install a specific release
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh -s -- --version v0.1.0

# install somewhere else
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh -s -- --dir ~/bin

# write the PATH line into your shell profile for you
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh -s -- --set-path

# restore the binary saved by the previous install
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh -s -- --rollback
```

Where it installs:

- **Termux:** `$PREFIX/bin`, which is already on `PATH`.
- **Everything else:** `~/.local/bin`, which you may need to add to `PATH`.

If the previous binary is replaced, the old one is kept as `zenx.old` next to it.
Put it back with:

```bash
./install.sh --rollback          # or: install.sh --dir <path> --rollback
```

Windows uses a PowerShell installer instead — see
[installation-windows.md](installation-windows.md).

## Method 2 — Cargo

```bash
cargo install zenx
```

**Status:** the `zenx` crate name is free on crates.io, but the package has not
been published yet, so this command does not work today. Until the first publish
(see `scripts/publish.sh`), use one of these:

```bash
# straight from the repository
cargo install --git https://github.com/sufiyan-sabeel/zenX zenx

# from a local clone
git clone https://github.com/sufiyan-sabeel/zenX
cargo install --path zenX/crates/zenx-cli
```

The package is named `zenx` and the executable it installs is `zenx`. In a
workspace repository, `cargo install --git` needs the package name spelled out;
that is why the first command ends with `zenx`.

Requirements: Rust 1.75 or newer and a C compiler, because SQLite is compiled
from source. On Termux that means `pkg install rust clang git` — see
[installation-termux.md](installation-termux.md).

Cargo installs into `~/.cargo/bin`, which is on `PATH` for any shell that has
sourced Cargo's environment file.

## PATH configuration

The installer prints the exact line when the directory is not on `PATH`. To do
it by hand, add one line to the profile your shell reads:

```bash
# Bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc

# Zsh
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc

# Termux
echo 'export PATH="$PREFIX/bin:$PATH"' >> ~/.bashrc
```

`--set-path` writes this for you and refuses to add a duplicate line: it checks
for the exact line first, so running the installer twice leaves one entry.

## Updating

```bash
zenx update --check    # is a newer release available?
zenx update            # download, verify, install
zenx update --dry-run  # download and verify, install nothing
zenx update --rollback # restore the binary saved by the last update
```

The updater replaces only a file named `zenx`, downloads only over `https`, and
verifies the archive against `SHA256SUMS.txt` before extracting it. The previous
binary is copied to `zenx.old` first, which is what `--rollback` restores.

If zenX was installed with Cargo, `zenx update` will not touch the binary: cargo
owns it. It tells you to run `cargo install zenx --force` (or the `--git` form)
instead.

## Uninstalling

```bash
zenx uninstall            # show what would be removed
zenx uninstall --yes      # remove the executable
zenx uninstall --yes --purge-data   # also delete ~/.zenx
```

The uninstaller only ever deletes a file named `zenx`, and it only deletes the
data directory when you pass both `--purge-data` and `--yes`. Your configuration,
sessions and skills stay otherwise.

For a Cargo install:

```bash
cargo uninstall zenx
rm -rf ~/.zenx    # only if you want the data gone too
```

## Verifying a download by hand

If you prefer to install manually, verify before you run anything:

```bash
curl -fsSLO https://github.com/sufiyan-sabeel/zenX/releases/latest/download/zenx-x86_64-unknown-linux-gnu.tar.gz
curl -fsSLO https://github.com/sufiyan-sabeel/zenX/releases/latest/download/SHA256SUMS.txt
sha256sum -c SHA256SUMS.txt --ignore-missing
tar -xzf zenx-x86_64-unknown-linux-gnu.tar.gz
install -m 755 zenx-x86_64-unknown-linux-gnu/zenx ~/.local/bin/zenx
```

## First run

```bash
zenx init
zenx status
```

`zenx init` creates `~/.zenx` (or `$ZENX_HOME`), writes a default `config.toml`
and creates the SQLite database. It never overwrites an existing configuration
without `--force`.
