# Changelog

All notable changes to zenX are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing yet.

## [0.1.0] - 2026-11-09

First release candidate.

### Added
- `install.sh` at the repository root, usable as
  `curl -fsSL .../install.sh | sh`. Detects OS and architecture, refuses
  unsupported platforms, verifies the archive against the release's
  `SHA256SUMS.txt` before extracting, keeps the previous binary as `zenx.old`,
  and can write an idempotent `PATH` line with `--set-path`.
- `install.ps1` for Windows: same fail-closed checksum verification through
  `Get-FileHash`, installs to `%LOCALAPPDATA%\zenX\bin`, optional `-SetPath`.
- `zenx update` with `--check`, `--tag`, `--dry-run` and `--rollback`.
  Downloads only over HTTPS, verifies SHA-256 before unpacking, replaces only a
  file named `zenx`, and defers to cargo when zenX was installed with cargo.
- `zenx uninstall`, which reports its plan first, deletes only the zenX
  executable, and needs `--purge-data --yes` before touching user data.
- The CLI package is now named `zenx` (directory unchanged) with the metadata
  crates.io requires, and internal dependencies carry versions so the whole
  workspace is publishable.
- `scripts/publish.sh`, which publishes the twelve crates in dependency order
  and supports `--dry-run`.
- CI jobs that run `cargo install --path crates/zenx-cli`, shellcheck the
  installers, install through `install.sh` from a locally built artifact, and
  assert that an unverifiable download is refused. The release workflow now
  verifies the installer against the real artifacts before publishing.
- Installation documentation: `docs/installation.md` and per-platform guides for
  Linux, macOS, Windows and Termux.
- A published `aarch64-linux-android` artifact, because Termux uses Bionic and
  cannot load a glibc build; `install.sh` and `zenx update` select it on Termux.
- `scripts/verify-metadata.py`, which checks crate names, versions, internal
  dependency versions and the crates.io publication order.


### Installation and distribution
- Unified `zenx` CLI with consistent exit codes and `--output json` on every
  command.
- `zenx-core`: versioned TOML configuration, unified error type, path
  resolution with traversal-safe joins, human/JSON reporter, structured
  logging.
- `zenx-storage`: SQLite database with ordered migrations keyed on
  `PRAGMA user_version`; refuses to open a newer schema than it understands.
- `zenx-security`: secret redaction for text and JSON payloads, SHA-256
  checksums, lexical path containment, safe archive entry validation,
  self-cleaning temporary directories.
- `zenx-trace`: sessions, sequenced events, search and filters, timeline
  inspection, JSON and Markdown export, JSONL import through an adapter trait,
  retention pruning, redaction on write.
- `zenx-skills`: install from git or a local directory, dry-run, conflict
  detection, manifest with checksum, verify, update, rollback, remove with
  backup, per-agent target directories.
- `zenx-doctor`: README, license, `.gitignore`, CI, git-state and
  documented-command checks; Rust, Node and Python detectors; isolated build
  runs; reviewable repair plans.
- `zenx-mobile`: Android/Termux detection, architecture and toolchain report,
  backup and restore of configuration, database and skills.
- `zenx-bench`: TOML task files, repeated measured runs, history, run
  comparison with regression detection.
- `zenx-contract`: OpenAPI 3 parsing in JSON and YAML, breaking-change
  classification, Markdown report, CI-friendly exit code.
- `zenx-docs`: documentation import with size limits, ranked local search with
  snippets, collection management.
- CI for Linux, macOS and Windows, an aarch64 cross-build, release workflow
  with SHA-256 checksums, and an installer script.

### Changed

- `docs/INSTALL.md` and `docs/TERMUX.md` were replaced by the guides above.

### Known limitations
- No dashboard yet; `zenx status` reports it as planned.
- Only the zenX JSONL trace adapter exists.
- `zenx skills update` requires git; local installs cannot be updated in place.
