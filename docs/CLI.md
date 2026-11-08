# CLI reference

Every command supports `--output human|json` and `--no-color`. JSON mode writes a
single document to stdout; everything else goes to stderr.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | success |
| 1 | a check failed, or breaking changes were found |
| 2 | invalid input or configuration |
| 3 | security violation (unsafe path, untrusted write) |
| 4 | not found |
| 5 | not implemented |
| 6 | aborted |
| 7 | an external command failed |

## Core

```
zenx init [--home DIR] [--force]
zenx status [--no-database]
zenx config [show|path|get KEY|set KEY VALUE]
```

`zenx status` prints the resolved home, configuration version, database schema
version and row counts, plus the honest status of every module.

## Trace

```
zenx trace [sessions] [--agent A] [--status S] [--search TEXT] [--limit N]
zenx trace inspect <session>
zenx trace export <session> [--format json|markdown] [--out FILE]
zenx trace record [--session ID] [--agent A] [--label L] [--file F] [--adapter NAME]
zenx trace start --agent A [--label L] [--cwd DIR]
zenx trace end <session> [--status completed]
zenx trace import <file> --agent A [--adapter NAME] [--label L]
zenx trace prune --days N
zenx trace adapters
zenx trace delete <session>
```

`record` reads JSONL from `--file` or stdin. One event per line:

```json
{"kind":"tool_call","name":"read_file","status":"ok","duration_ms":12,"payload":{},"files":["src/main.rs"]}
```

Kinds: `message`, `tool_call`, `tool_result`, `file_change`, `decision`, `error`,
`custom`. Payloads are redacted on write when `trace.redact` is true (the
default).

## Skills

```
zenx skills [list]
zenx skills search <query>
zenx skills install <git-url-or-path> [--name N] [--dry-run] [--force] [--allow-network] [--no-targets]
zenx skills verify <name>
zenx skills update <name>
zenx skills rollback <name>
zenx skills remove <name>
```

`install` accepts a git URL or a local directory containing `SKILL.md`.
`--dry-run` prints the plan and writes nothing. `search` searches installed
skills; there is no remote registry.

## Doctor

```
zenx doctor [PATH] [--fix] [--apply] [--build] [--tests] [--report FILE]
```

`--fix` prints the repair plan. `--apply` performs it, and only ever creates
files that do not exist. `--build` and `--tests` run the ecosystem build in a
temporary target directory outside the repository.

## Mobile

```
zenx mobile [diagnose]
zenx mobile backup [--out DIR]
zenx mobile restore <backup-dir> [--overwrite]
```

## Bench

```
zenx bench [run] [TASKFILE] [--repeat N] [--label L]
zenx bench compare <base-run> <head-run>
zenx bench history [--limit N]
```

Task file format (`zenx-bench.toml`):

```toml
[[task]]
id = "version-prints"
command = ["zenx", "--version"]
timeout_secs = 30
expect_exit = 0
expect_stdout_contains = "zenx"
```

`command` is an argument vector. No shell is involved, so shell syntax in a task
file does not work — and cannot be abused.

## Contract

```
zenx contract compare <base> <head> [--out FILE]
```

Exits 1 when breaking changes are found, so CI can gate on it.

## Docs

```
zenx docs [search] <query> [--collection C] [--version V] [--limit N]
zenx docs add <directory> [--collection C] [--version V]
zenx docs list
zenx docs remove <collection> [--version V]
```
