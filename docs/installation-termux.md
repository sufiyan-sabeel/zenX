# Installing on Android with Termux

Termux is a first-class target. Everything below was designed around its
constraints: no root, no glibc, aarch64, and a package manager you drive
yourself.

## Use the right Termux

Install Termux from **F-Droid** or the **GitHub releases**. The Play Store build
is outdated and 32-bit, which breaks most Rust crates and has no matching zenX
release.

## Option 1 — prebuilt binary

```bash
pkg update
pkg install curl tar -y
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh
```

The script recognises Termux (through `$TERMUX_VERSION` or `$PREFIX`) and
installs to `$PREFIX/bin`, which is already on `PATH` — no profile edit needed.

It verifies the archive against the release's `SHA256SUMS.txt` before extracting
it, and refuses to continue if the checksums are missing or do not match.

## Option 2 — build from source

```bash
pkg update
pkg install rust clang git -y
cargo install --git https://github.com/sufiyan-sabeel/zenX zenx
```

`clang` is not optional. SQLite is compiled from source by `rusqlite`'s bundled
feature, which needs a C compiler — that is exactly what makes zenX work on
Android without a system SQLite.

Cargo installs to `~/.cargo/bin`. If `zenx` is not found afterwards:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Add that line to `~/.bashrc` to keep it. Running the same command twice would add
it twice, so check first:

```bash
grep -q 'cargo/bin' ~/.bashrc || echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
```

## Check the environment

```bash
zenx mobile diagnose
```

Reports the OS, architecture, Termux version, Android release and every
toolchain zenX cares about, with the `pkg install` line for anything missing. It
never runs `pkg` and never asks for root.

## Backups

```bash
zenx mobile backup                       # into ~/.zenx/backups
zenx mobile backup --out /sdcard/zenx-backups
zenx mobile restore ~/.zenx/backups/zenx-backup-20260101T120000Z --overwrite
```

A backup is a plain directory: `config.toml`, `zenx.db`, `skills/` and a
`manifest.json`. Copy it to shared storage with `termux-setup-storage`.

## Updating

```bash
zenx update --check
zenx update
```

A Cargo install is not replaced by the updater — it tells you to run
`cargo install --git https://github.com/sufiyan-sabeel/zenX zenx` again, because
cargo owns that binary.

## Uninstalling

```bash
zenx uninstall --yes                 # removes $PREFIX/bin/zenx
zenx uninstall --yes --purge-data    # also removes ~/.zenx
```

## Known limitations on Android

- A full workspace build on a phone takes minutes, not seconds.
- `zenx mobile restore` needs `--overwrite` to replace existing state, because
  replacing a live database is not recoverable.
- Benchmarks measure the phone, including thermal throttling. Compare runs taken
  under similar conditions.
- 32-bit Termux is not supported: there is no prebuilt archive for it and most
  dependencies do not build there.
