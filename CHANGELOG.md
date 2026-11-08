# Changelog

All notable changes to zenX are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing yet.

## [0.1.0]

First working release.

### Added

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

### Known limitations

- No dashboard yet; `zenx status` reports it as planned.
- Only the zenX JSONL trace adapter exists.
- `zenx skills update` requires git; local installs cannot be updated in place.
