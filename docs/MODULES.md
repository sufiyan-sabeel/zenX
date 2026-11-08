# Module guide

Each module is a separate crate with a real implementation and tests. They share
`zenx-core` (errors, config, paths, output) and, where they persist data,
`zenx-storage`.

## zenX Trace (`crates/zenx-trace`)

**Model.** A *session* is one agent run: `id`, `agent`, optional `label`, `cwd`,
`status`, start and end timestamps. An *event* belongs to a session and carries a
monotonic `seq`, a `kind`, a `name`, an optional `duration_ms`, a `status` and a
JSON `payload` (which absorbs `files` when the adapter reports them).

**Why sequence numbers.** Wall-clock timestamps collide and clocks drift; `seq`
is assigned inside the insert, so a timeline always reconstructs in order even
when two events share a millisecond.

**Redaction.** When `trace.redact` is true (default), payloads pass through
`zenx_security::redact_json` before the insert. Keys such as `password`, `token`,
`secret`, `api_key`, `authorization` and `credential` are replaced wholesale;
string values are scanned for known token shapes.

**Adapters.** `zenx trace adapters` lists what this build implements. Today that
is `generic-jsonl`, zenX's own format. Adding a real adapter for another agent
means writing the parser, committing a sample transcript, and adding tests —
nothing else. Adapters that are merely planned are absent from the registry, and
`zenx trace import --adapter claude-code` fails with the list of what does exist.

## zenX Skills (`crates/zenx-skills`)

**Store model.** Skills live in one store (`<home>/skills/<name>`), owned by
zenX. Agent directories receive copies, configured as `skills.targets` with a
`layout` of `skill-dir` (whole directory) or `flat` (just `SKILL.md`).

**Install pipeline.** Stage into a temporary directory → find the directory
holding `SKILL.md` → parse front matter → validate the name (no dots at the
start, no path separators) → checksum `SKILL.md` → check for conflicts → back up
any existing copy → copy the tree (skipping `.git`) → write
`.zenx-manifest.json` → record in SQLite → copy into targets.

**What the manifest buys you.** `zenx skills verify` recomputes the checksum and
reports drift plus any files the manifest lists that are now missing. `rollback`
restores the most recent backup recorded in `skill_backups`.

**Refusals.** A directory that zenX did not create has no manifest, so `remove`
and target overwrites refuse it. That is deliberate: a hand-written skill
directory is somebody's work.

## zenX Doctor (`crates/zenx-doctor`)

**Detectors.** Rust (`Cargo.toml`), Node (`package.json`) and Python
(`pyproject.toml`, `setup.py`, `setup.cfg`, `requirements.txt`). A detector only
runs when its manifest exists.

**Common checks.** README present; every command in a README fenced block
resolves (`zenx` subcommands are validated against the real command list, other
programs against `PATH`); license file present and recognised; `.gitignore`
present; CI configuration present; git branch and dirty state.

**Builds are opt-in and isolated.** `--build` runs `cargo check` with
`CARGO_TARGET_DIR` pointing at a temporary directory, so a diagnostic run leaves
no `target/` behind. `--tests` does the same for `cargo test --no-run`,
`npm test`, and `pytest --collect-only`.

**Repairs.** `--fix` prints the plan; `--apply` performs it. Repairs only create
paths that do not exist (README skeleton, `.gitignore`, `tests/`), so a review
is meaningful: nothing is overwritten.

## zenX Mobile (`crates/zenx-mobile`)

Detection reads environment variables (`TERMUX_VERSION`, `PREFIX`), checks for
`/system/build.prop` to identify Android, reads the release string from it, and
probes each toolchain by running `--version`. Missing tools are reported with
the `pkg install` line the user could run; zenX never runs it.

Backup writes a plain directory (not an opaque archive) so it can be inspected
and copied with any file manager. Restore validates `manifest.json` first and
refuses to overwrite existing state without `--overwrite`.

## zenX Bench (`crates/zenx-bench`)

**Task file.** `[[task]]` entries with `id`, `command` (argument vector),
optional `timeout_secs`, `expect_exit`, `expect_stdout_contains`.

**Execution.** The child is spawned with piped stdout/stderr drained on their own
threads (so a chatty task cannot deadlock on a full pipe), polled for exit, and
killed on timeout.

**Results.** Each iteration is a row in `bench_results`. Completion rate is
`passed / iterations`; duration statistics are computed from measured
wall-clock times only.

**Comparison.** A task regresses when completion drops, when it disappears from
the head run, or when completion is unchanged but the mean duration is 20% or
more slower.

## zenX Contract (`crates/zenx-contract`)

**Parsing.** JSON first, YAML second. Path-level and operation-level parameters
are merged; response schemas are canonicalised to JSON so any structural change
is detectable.

**Breaking rules.** Removed endpoint; removed response code; added *required*
parameter; parameter made required; request body made required; any change to a
response schema; parameter schema type change.

**Non-breaking.** Added endpoint; added optional parameter; removed parameter;
added response code; parameter made optional; same-type parameter schema change.

**Informational.** Endpoint newly marked deprecated.

`--out` writes a Markdown table suitable for a pull request comment.

## zenX Docs (`crates/zenx-docs`)

Import walks a directory for `.md`, `.markdown`, `.txt` and `.rst`, skipping
files over `docs.max_file_bytes`. The title is the first heading, else the first
non-empty line, else the path. Re-importing a collection version replaces it.

Search tokenises the query, scans title and body, scores title hits five times
higher, and returns a snippet around the first match. Ranking happens in Rust
rather than in SQLite FTS5 so behaviour is identical on every platform and needs
no special build flag.
