# Roadmap

This file states what exists, what does not, and what would make the next
release. Nothing here is advertised in the CLI until it is implemented.

## Shipped in 0.1.0

- Core engine, versioned configuration, SQLite storage with migrations.
- Security primitives with hostile-input tests.
- Trace: sessions, events, search, inspection, JSON/Markdown export, JSONL
  import, pruning, redaction.
- Skills: install, dry-run, verify, update, rollback, remove with backup,
  agent targets.
- Doctor: common checks, Rust/Node/Python detectors, isolated builds, reviewable
  repairs.
- Mobile: Android/Termux detection, toolchain report, backup and restore.
- Bench: measured runs, history, comparison with regression detection.
- Contract: OpenAPI 3 JSON/YAML parsing, breaking-change classification.
- Docs: import, ranked offline search, collection management.

## Next: 0.2.0 — the dashboard

- A React + TypeScript dashboard served by a local backend from the same binary,
  reading the same SQLite file. Real data only: no fabricated charts, no buttons
  without actions.
- Pages: overview, trace, skills, doctor, mobile, bench, contracts, docs,
  settings.
- Loading and error states, responsive layout, graphite surfaces with a muted
  teal accent, Lucide icons.

## Next: 0.3.0 — real agent adapters

- Claude Code and Codex transcript adapters, each with a committed sample and
  tests.
- Skill-directory adapters for agents whose layout differs beyond `skill-dir`
  and `flat`.
- `zenx trace` live tailing of an in-progress session.

## Later

- Self-update with checksum verification, replacing the install script.
- Distributed benchmark runs with a shared result file for comparison.
- Plugin loading through a stable out-of-process protocol rather than a C ABI.
- GraphQL and gRPC contract comparison beyond OpenAPI.

## Explicitly not planned

- A hosted service. zenX is local-first by design.
- Telemetry of any kind.
- Fabricated benchmark scores or "estimated" performance numbers presented
  alongside measured ones.
