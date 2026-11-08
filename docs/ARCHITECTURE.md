# Architecture

## Shape

```
zenx-cli  ──▶ zenx-trace, zenx-skills, zenx-doctor, zenx-mobile,
              zenx-bench, zenx-contract, zenx-docs
                     │
                     ├──▶ zenx-core        (errors, config, paths, output, logging)
                     ├──▶ zenx-storage     (SQLite, migrations)
                     └──▶ zenx-security    (paths, redaction, checksums, temp dirs)
```

Dependencies only point down. No module crate depends on another module crate,
and none of them depend on the CLI, so a module can be used as a library.

## Why a single database

Sessions, skills, benchmark history, documentation and contract runs all live in
`zenx.db`. One file to back up, one schema version to migrate, one place to look
when something is wrong. Migrations are an ordered list in
`crates/zenx-storage/src/schema.rs`, applied against `PRAGMA user_version`; an
applied entry is never edited, only appended to.

## Output discipline

`Reporter` decides where text goes:

- `json(value)` — the single machine-readable document on stdout;
- `line(text)` — human output on stdout, suppressed in JSON mode;
- `note(text)` — diagnostics on stderr, always.

That is why `--output json` can be piped into `jq` without filtering noise.

## Error discipline

`ZenxError` has one variant per failure class, each with a documented exit code.
`main` prints the error once — as a JSON object in JSON mode — and exits with
that code. Nothing calls `process::exit` deep inside a module, so tests can
assert on errors instead of process state.

## Filesystem discipline

- User-supplied relative paths go through `safe_join`, which *rejects* `..`
  instead of normalising it.
- Copying a tree skips symlinks (`copy_dir`) or skips `.git` (`copy_skill_tree`).
- Temporary work happens in `TempDir`, which removes itself on drop, including
  on the error path.
- Destructive operations back up first and require an explicit flag.

## Subprocess discipline

No shell. `git`, `cargo`, `npm`, `pytest` and benchmark commands are started with
argument vectors. `git` runs with `GIT_TERMINAL_PROMPT=0` and
`GIT_ASKPASS=true` so a private URL fails instead of hanging on a prompt.

## Cross-platform

Platform differences are confined to small `#[cfg]` functions (home directory
resolution) and to path handling that normalises `\` for display. Nothing assumes
a desktop session, a `systemd` unit or a system package manager. Termux is the
reason SQLite is built from source via `rusqlite`'s bundled feature.

## Testing strategy

- Unit tests live beside the code and cover the security-relevant paths: path
  traversal, archive entries, redaction, checksums, schema migration, diffing.
- CLI tests in `crates/zenx-cli/tests/cli.rs` run the real binary in a throwaway
  `ZENX_HOME` and assert on JSON output and exit codes.
- CI runs both on Linux, macOS and Windows, and cross-builds aarch64.
