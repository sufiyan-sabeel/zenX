# Configuration

`config.toml` lives in the zenX home directory (`$ZENX_HOME`, default
`~/.zenx`). `zenx init` writes it; `zenx config` reads and edits it.

```toml
version = 1

[output]
format = "human"   # or "json"
color = true

[trace]
redact = true
capture_file_changes = true
retention_days = 30   # omit or set to "none" to keep sessions forever

[skills]
allow_network = true
# store = "/some/other/path"

[[skills.targets]]
name = "claude-code"
path = "/home/you/.claude/skills"
layout = "skill-dir"

[[skills.targets]]
name = "codex"
path = "/home/you/.codex/skills"
layout = "flat"

[docs]
max_file_bytes = 2097152
```

## Rules the loader enforces

- `version` may not be newer than the binary understands. An older binary
  refuses to guess at a newer schema.
- `output.format` must be `human` or `json`.
- Every `skills.targets` entry must have `layout` of `skill-dir` or `flat`, and
  `path` must be absolute.
- Unknown keys are ignored, and missing keys fall back to defaults, so a config
  written by an older release keeps working.

## Editing it

```bash
zenx config show
zenx config path
zenx config get trace.retention_days
zenx config set trace.retention_days 30
zenx config set output.format json
```

`set` validates before writing and writes atomically (temp file then rename), so
an interrupted edit cannot leave a truncated config behind.

Editable keys: `output.format`, `output.color`, `trace.redact`,
`trace.capture_file_changes`, `trace.retention_days`, `skills.allow_network`,
`skills.store`, `docs.max_file_bytes`. Target directories are edited in the file
directly, because they are a list.

## Environment variables

| Variable | Effect |
|----------|--------|
| `ZENX_HOME` | Overrides the home directory |
| `ZENX_LOG` | `off`, `error`, `warn` (default), `info`, `debug` — structured JSON logs on stderr |
| `NO_COLOR` | Respected by the reporter |

## Precedence

`--output` on the command line beats `output.format` in the file, which beats the
built-in default of `human`.
