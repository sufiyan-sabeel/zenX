# zenX v0.1.0

**Build smarter. Debug deeper. Develop anywhere.**

The first release: one Rust CLI, one SQLite database, seven modules that share a
core engine and work offline.

## Install

Linux and Termux, checksum-verified:

```bash
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.ps1 | iex
```

From source, which is also the only way on platforms without a prebuilt archive:

```bash
cargo install --git https://github.com/sufiyan-sabeel/zenX zenx
```

`cargo install zenx` from crates.io is **not available yet**: the crate name is
reserved for it, but the package has not been published. This document will say
so until it is.

## What is in it

- **Trace** — record agent sessions and events in SQLite, inspect timelines,
  export JSON or Markdown, import JSONL, redact secrets before writing.
- **Skills** — install from git or a local directory, verify checksums, update,
  roll back, remove with a backup, copy into agent-specific directories.
- **Doctor** — check README, license, `.gitignore`, CI, git state and documented
  commands, plus Rust/Node/Python detectors; builds run outside the repository;
  repairs are planned and only applied on request.
- **Mobile** — Android/Termux detection, toolchain report, backup and restore.
  No root, ever.
- **Bench** — TOML task files, repeated measured runs, run comparison with
  regression detection. Measured numbers only.
- **Contract** — OpenAPI 3 comparison in JSON or YAML, breaking-change
  classification, CI-friendly exit code.
- **Docs** — import Markdown, ranked offline search with snippets.

Plus the installation lifecycle: `install.sh`, `install.ps1`, `zenx update`
(checksum-verified, `--check`, `--dry-run`, `--rollback`) and `zenx uninstall`,
which never deletes your data without `--purge-data --yes`.

## Platforms

See [platforms.md](platforms.md) for the matrix of what is built, what has been
run, and what has not. In short: Linux x86_64 and aarch64, macOS Intel and Apple
Silicon, Windows x86_64 and Termux aarch64 are all supported; Termux is verified
as a build, not yet executed on a device by the maintainers.

## Known limitations

- The dashboard is designed but not implemented. `zenx status` reports it as
  planned, and there is no command for it.
- Only the zenX JSONL trace adapter exists. Adapters that are not implemented do
  not appear in `zenx trace adapters`.
- `zenx skills update` needs git; a skill installed from a local directory cannot
  be updated in place.
- Windows cannot delete a running program, so `zenx uninstall` renames the
  executable and asks you to remove the leftover file.
- macOS binaries are not signed or notarised; install through the script (which
  uses `curl`) or clear the quarantine attribute.

## Verifying a download

Every release archive is listed in `SHA256SUMS.txt`:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Both installers refuse to install anything whose checksum is missing or does not
match.
