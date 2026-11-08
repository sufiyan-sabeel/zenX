# Security policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | yes       |

## Reporting a vulnerability

Open a private security advisory at
<https://github.com/sufiyan-sabeel/zenX/security/advisories/new>. Do not open a
public issue for anything exploitable.

Please include the version (`zenx --version`), the platform, the exact command,
and what an attacker gains. A proof of concept helps; a patch is welcome but not
required.

Expect an acknowledgement within 7 days. Fixes for confirmed issues ship in a
patch release, and the advisory names the affected versions.

## What zenX already does

- **Path safety.** Every user-supplied relative path goes through
  `zenx_core::paths::safe_join`, which rejects absolute paths and `..`
  components rather than normalising them.
- **Archive validation.** `zenx_security::archive` rejects absolute entries,
  drive prefixes, backslashes used as traversal and duplicate targets.
- **Redaction.** Trace payloads are redacted (keys such as `password`, `token`,
  `secret`, `authorization`, and value patterns such as `sk-…`, `ghp_…`,
  `AKIA…`, bearer tokens and PEM private keys) before they are written to the
  database.
- **No shell interpolation.** Subprocesses are always started with an argument
  vector; no command string is built from user input.
- **No silent destruction.** Removal and replacement back up first; unmanaged
  directories are refused; repairs and restores require explicit flags.
- **No network without consent.** Git installs require
  `skills.allow_network = true` or `--allow-network`.
- **No credentials in the repository.** Nothing is hardcoded; there is no
  telemetry and no phone-home code.

## What zenX cannot protect you from

- A skill you install from a source you do not trust: skills are copied as
  files, and their scripts run when your agent runs them.
- A benchmark task file you did not read: `zenx bench run` executes the commands
  in it by design.
- A redaction rule that misses a secret format nobody has added yet. Redaction is
  a safety net, not a guarantee; do not pipe production credentials through an
  agent transcript.
