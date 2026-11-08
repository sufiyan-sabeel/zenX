# Android and Termux

Termux is a first-class target, not an afterthought. Everything in this document
was designed around its constraints: no root, no systemd, no glibc, aarch64, and
a package manager you drive yourself.

## Install Termux correctly

Use the **F-Droid** or **GitHub** release of Termux. The Play Store build is
outdated and 32-bit, which breaks most Rust crates.

```bash
pkg update
pkg install rust clang git
```

`clang` is not optional: `rusqlite`'s bundled SQLite is C code that must be
compiled locally.

## Build zenX

```bash
git clone https://github.com/sufiyan-sabeel/zenX
cd zenX
cargo build --release
install -m 755 target/release/zenx "$PREFIX/bin/zenx"
```

`$PREFIX/bin` is already on `PATH` in Termux, so no profile edits are needed.

## Check the environment

```bash
zenx mobile diagnose
```

It reports the OS, architecture, Termux version, Android release, and every
toolchain zenX cares about, with the `pkg install` line for anything missing. It
never runs `pkg` for you and never asks for root.

## Backups

```bash
zenx mobile backup                 # into ~/.zenx/backups
zenx mobile backup --out /sdcard/zenx-backups
zenx mobile restore ~/.zenx/backups/zenx-backup-20260101T120000Z --overwrite
```

A backup is a plain directory containing `config.toml`, `zenx.db`, `skills/` and
a `manifest.json`. Copy it to shared storage with `termux-setup-storage` and a
file manager, or sync it with `rsync` over SSH.

## Known limitations on Android

- `zenx doctor --build` and `--tests` work, but a full Rust workspace build on a
  phone is slow; expect minutes, not seconds.
- `zenx mobile restore` refuses to overwrite existing state without
  `--overwrite`, because replacing a live database is not recoverable.
- Benchmarks measure the phone, including thermal throttling. Compare runs taken
  under similar conditions, not across a hot and a cold device.
- The dashboard is not implemented; when it is, it will be served by the local
  binary so no Node.js toolchain is needed on the device.
