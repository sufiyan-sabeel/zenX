# Troubleshooting

## "could not determine the home directory"

`HOME` (or `USERPROFILE` on Windows) is not set, which happens in minimal
containers and some Termux sessions started oddly. Set it explicitly:

```bash
export ZENX_HOME="$PWD/.zenx"
zenx init
```

## "database schema version N is newer than this build supports"

You used a newer zenX, then ran an older one. Upgrade the binary. Do not delete
the database to silence the message — it contains your sessions and skills.

## Skill install says "network access is disabled"

Either set `skills.allow_network = true`:

```bash
zenx config set skills.allow_network true
```

or pass `--allow-network` for a single install. This flag exists so a script
cannot quietly fetch code.

## Skill install fails with "no SKILL.md in … or its immediate subdirectories"

Point the command at the directory that contains `SKILL.md`. If a repository
holds several skills, name the specific one:

```bash
zenx skills install https://github.com/example/skills --name pdf-tools
```

For a local checkout, pass the skill's own subdirectory.

## "several skills found"

The repository has more than one skill at its top level and zenX will not guess.
Pass the subdirectory that holds the skill you want.

## `zenx skills verify` reports drift

The files changed after install. That is the check working. Either accept it
(edit the manifest by reinstalling with `--force`) or restore the previous copy
with `zenx skills rollback <name>`.

## A skill directory is refused as "not installed by zenX"

It has no `.zenx-manifest.json`. zenX will not delete or overwrite a directory it
did not create. Move it away yourself, or install with `--force` if you know it
is disposable.

## `zenx doctor --build` fails but `cargo build` works

`doctor` builds with `CARGO_TARGET_DIR` set to a temporary directory. If your
project depends on artifacts already sitting in `./target`, that build starts
cold. It also uses `cargo check --all-targets`, which compiles tests and benches
that a plain build skips.

## `zenx trace record` seems to hang

It is reading stdin. Either pipe something in, or pass `--file`:

```bash
zenx trace record --agent codex --file transcript.jsonl
```

## Imported events are missing

Lines that fail to parse are counted, not silently dropped. The summary shows
`skipped`, and `ZENX_LOG=warn` prints the line number and reason:

```bash
ZENX_LOG=warn zenx trace record --agent codex --file broken.jsonl
```

## Termux build fails in `libsqlite3-sys`

There is no C compiler. Install one:

```bash
pkg install clang
```

SQLite is compiled from source by `rusqlite`'s bundled feature, which is what
makes zenX work on Android at all.

## Benchmark says a task timed out

The default is 120 seconds per iteration; set `timeout_secs` in the task file. A
task that cannot start at all is reported as an external command failure, not a
timeout — check the program name in `command`.

## `zenx contract compare` exits 1 in CI

That is the intended signal: breaking changes were found. The JSON output has
`breaking` and a `changes` array with `severity`, `kind`, `endpoint` and
`detail`.
