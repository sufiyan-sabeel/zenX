# Security model

## Trust boundaries

| Input | Trusted? | Handling |
|-------|----------|----------|
| Command-line arguments | partly | Validated; paths rejected if absolute or traversing |
| `config.toml` | partly | Validated on load; unknown keys ignored |
| Skill source (git URL or directory) | no | Staged, name validated, checksummed, manifest written |
| Agent transcript (JSONL) | no | Parsed, kind validated, payload redacted before storage |
| Archive entry names | no | Rejected if absolute, prefixed or traversing (zip-slip) |
| Benchmark task file | no | Executed by design — that is the feature, and the docs say so |
| OpenAPI documents | no | Parsed as data only; no code execution |
| Documentation corpus | no | Read as text, size-capped, stored in SQLite |

## Redaction

Applied to trace payloads when `trace.redact` is on (default):

- **Whole values** for keys containing `password`, `passwd`, `secret`, `token`,
  `api_key`, `apikey`, `access_key`, `private_key`, `authorization`,
  `credential`.
- **Pattern matches** for `sk-…`/`rk-…` keys, `ghp_…`/`github_pat_…` tokens,
  `AKIA…` access key ids, `xox[baprs]-…` Slack tokens, `Bearer …` headers,
  `key=value` pairs where the key names a credential, and PEM private key blocks.

Redaction is a safety net, not a guarantee. If a secret format is missing, add a
rule and a test in `crates/zenx-security/src/redact.rs`.

## Consent

- Git installs require `skills.allow_network = true` or `--allow-network`; the
  refusal message names the setting.
- `zenx doctor` never builds unless `--build`/`--tests` is passed, and builds
  into a temporary target directory outside the repository.
- `zenx doctor --fix` prints a plan; `--apply` is a separate decision.
- `zenx mobile restore` needs `--overwrite` to replace existing state.
- Skill replacement backs up the previous copy and reports where.

## What is never done

- No `sudo`, no root, no package manager invocation.
- No shell string construction from user input.
- No credential storage: zenX has no accounts, no tokens, no telemetry.
- No deletion of a directory zenX did not create.
- No writing into a repository during a diagnostic run.

## Reporting

See [SECURITY.md](../SECURITY.md) in the repository root.
