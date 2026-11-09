# zenX

**Build smarter. Debug deeper. Develop anywhere.**

zenX is a local-first developer infrastructure platform in Rust: one CLI, one
SQLite database, seven modules that share a core engine. It is built for people
who work across desktop Linux, macOS, Windows and Android/Termux, and who want
agent tooling that keeps working offline.

```bash
zenx init
zenx doctor .
zenx trace sessions
```

## Modules

| Module | Command | What it actually does |
|--------|---------|-----------------------|
| zenX Trace | `zenx trace` | Records agent sessions and events in SQLite, inspects timelines, exports JSON/Markdown, imports JSONL transcripts, redacts secrets before writing |
| zenX Skills | `zenx skills` | Installs skills from git or a local directory, verifies checksums, updates, rolls back, removes with a backup, copies into agent-specific directories |
| zenX Doctor | `zenx doctor` | Checks README, license, `.gitignore`, CI, git state, documented commands, and per-ecosystem manifests; optional builds run in an isolated target dir; repairs are planned, never implicit |
| zenX Mobile | `zenx mobile` | Detects Android/Termux, architecture and toolchains; backs up and restores configuration, database and skills; never needs root |
| zenX Bench | `zenx bench` | Runs task files repeatedly, records measured completion rates and durations, compares two runs and flags regressions |
| zenX Contract | `zenx contract` | Parses OpenAPI 3 (JSON or YAML), compares two versions, classifies breaking changes, exits non-zero for CI |
| zenX Docs | `zenx docs` | Imports Markdown/text documentation, ranks local full-text search results, works entirely offline |

A local React dashboard over the same database is designed but **not built yet**.
`zenx status` reports it as `planned`, and there is no command pretending
otherwise.

## Install

Prebuilt binaries are published for Linux (x86_64, aarch64), macOS (Intel and
Apple Silicon) and Windows (x86_64). The installer verifies the download against
the release's `SHA256SUMS.txt` before unpacking anything:

```bash
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh
```

On Windows, use the PowerShell installer instead:

```powershell
irm https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.ps1 | iex
```

Or build with Cargo — from the repository, until the crate is published:

```bash
cargo install --git https://github.com/sufiyan-sabeel/zenX zenx
```

`cargo install zenx` is the intended final form; the crate name is reserved for
it but **the package is not on crates.io yet**, so that command does not work
today.

Android/Termux needs `pkg install rust clang git` first, because SQLite is
compiled from source. Full instructions: [docs/installation.md](docs/installation.md).
The platform matrix — what is built, what has actually been executed on a
machine, and what has not — is in [docs/platforms.md](docs/platforms.md).

## Quick start

```bash
# 1. create the state directory, config and database
zenx init

# 2. check a repository
zenx doctor . --build

# 3. record an agent session from a JSONL stream
cat examples/trace/session.jsonl | zenx trace record --agent codex --label demo
zenx trace sessions
zenx trace inspect sess_xxxxxxxxxxxx

# 4. install a skill from this repository's example
zenx skills install examples/skills/demo-skill --dry-run
zenx skills install examples/skills/demo-skill
zenx skills verify demo-skill

# 5. import documentation and search it offline
zenx docs add examples/docs --collection zenx
zenx docs search release

# 6. check an API for breaking changes (exit code 1 when breaking)
zenx contract compare examples/contracts/base.yaml examples/contracts/head.yaml

# 7. keep the binary current
zenx update --check
```

Every command accepts `--output json` for automation. JSON mode writes exactly
one document to stdout; diagnostics go to stderr.

## Where state lives

One directory, `$ZENX_HOME` or `~/.zenx`:

```
config.toml   versioned configuration
zenx.db       SQLite database (WAL)
skills/       installed skills, one directory per skill
backups/      skill backups and mobile backups
tmp/          scratch space, cleaned up per operation
```

`zenx mobile backup` copies the first three into a timestamped directory, and
`zenx mobile restore` puts them back — with an explicit `--overwrite` when state
already exists.

## Design rules this codebase holds itself to

- **No fake functionality.** A module appears in `zenx status` as `implemented`
  only when its crate has real logic and tests. Nothing advertises an agent
  adapter that has not been written.
- **No silent destruction.** Replacing a skill backs up the previous copy first.
  Repairs need `--fix --apply`. Restores need `--overwrite`. Unmanaged
  directories are never deleted.
- **Measured, not estimated.** `zenx bench` reports only numbers from processes
  it started and waited for. A task that cannot run fails; it does not become a
  flattering skip.
- **Untrusted input stays untrusted.** Skill archives, transcript payloads and
  user paths pass through `zenx-security` before touching the filesystem, and
  payloads are redacted before they reach the database.
- **Diagnostics are read-only.** Builds triggered by `zenx doctor` write to a
  temporary `CARGO_TARGET_DIR` outside the repository.

## Documentation

- [Installation](docs/installation.md) — installer and Cargo, verification, PATH, uninstall
- [Linux](docs/installation-linux.md)
- [macOS](docs/installation-macos.md)
- [Windows](docs/installation-windows.md)
- [Android and Termux](docs/installation-termux.md) · [Termux smoke test](docs/termux-smoke-test.md)
- [Platform matrix](docs/platforms.md)
- [CLI reference](docs/CLI.md)
- [Module guide](docs/MODULES.md)
- [Configuration](docs/CONFIGURATION.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Security model](docs/SECURITY.md)
- [Writing a module or adapter](docs/PLUGINS.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Roadmap](docs/ROADMAP.md)

## Development

```bash
bash scripts/dev-check.sh    # rustfmt, clippy -D warnings, tests, doc-command check
cargo test --workspace --all-targets
```

CI runs the same checks on Linux, macOS and Windows, cross-builds for
`aarch64-unknown-linux-gnu` (the Termux target) and verifies that every `zenx`
subcommand mentioned in the documentation exists in the binary.

## Project status

Version 0.1.0. Implemented and covered by tests: core engine, storage,
security primitives, all seven modules listed above, plus the installation
lifecycle — `install.sh`, `install.ps1`, `zenx update` (checksum-verified, with
`--rollback`) and `zenx uninstall`. Not implemented: the React dashboard, agent
adapters beyond zenX's own JSONL format, and publication to crates.io. Those are
listed in [docs/ROADMAP.md](docs/ROADMAP.md); the dashboard has no command at
all rather than a stub.

## License

Apache-2.0. See [LICENSE](LICENSE).
