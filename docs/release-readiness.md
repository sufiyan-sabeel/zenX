# v0.1.0 release readiness

Evidence gathered on 2026-11-09 from GitHub Actions, not from local assumption.
Nothing here was published: no tag, no release, no crates.io upload.

## Commit

| | |
|---|---|
| Code commit verified | `368fec5` (full SHA in the Actions run below) |
| Branch | `main` |
| Working tree at verification | clean |
| Version | `0.1.0` in `[workspace.package]`, inherited by all 12 crates |
| Tags | none |

## Workflow runs

| Workflow | Run | Result |
|----------|-----|--------|
| CI | 42573178870 | success, 10/10 jobs |
| Verify | 42573178950 | success, 10/10 jobs |
| Release candidate | 42573178872 | 11 of 12 jobs success; macOS runtime cancelled (no runner) |

## Tests

`cargo test --workspace --all-targets`, from the logs of the Verify run:

| Platform | Passed | Failed |
|----------|-------:|-------:|
| Ubuntu | 150 | 0 |
| macOS | 150 | 0 |
| Windows | 147 | 0 |

Windows runs three fewer tests because `#[cfg(unix)]` cases do not compile there.
`cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D
warnings` are clean on all three.

## Artifacts built by the candidate workflow

| Archive | SHA-256 |
|---------|---------|
| `zenx-x86_64-unknown-linux-gnu.tar.gz` | `94fcda452c5a84476972ec583c125f3133a8597605f75b3a5c4f0ffda9faa9ef` |
| `zenx-aarch64-unknown-linux-gnu.tar.gz` | `eb037efc45f7336876b499db0c7dcee0250190de45461dc2bd7051a54fd3f1ef` |
| `zenx-x86_64-apple-darwin.tar.gz` | `0d7d806bfef7ec85b55c7350ef99e3b5898e01187553e9ce377420176b248c08` |
| `zenx-aarch64-apple-darwin.tar.gz` | `fefe98e693aa2566f49dc6063e1329b1ec4494602d9e29cf6aeba81433da9a8e` |
| `zenx-x86_64-pc-windows-msvc.zip` | `6f07baeb094288e49cb1eec3006bb4987559a01a2fd1b8a457766d7c5bec5c4e` |
| `zenx-aarch64-linux-android.tar.gz` | `231f7167beb628e6fe466c2fd2ccf0a2b7f125e751b5bbf66bc8084bc4dde19e` |

These are the checksums of the candidate build. A tagged release rebuilds every
archive, so the published `SHA256SUMS.txt` will carry different values; verify
against the file attached to that release, never against this table.

## Runtime results

Nine checks per platform: `--version`, `--help`, `doctor`, `update --check`,
`update --dry-run`, `update`, `update --rollback`, `uninstall` plan, and
`uninstall --yes` leaving user data intact.

| Platform | Installer | Checks | Notes |
|----------|-----------|--------|-------|
| Linux x86_64 | `install.sh` | 9/9 passed | checksum verified before extraction |
| Linux aarch64 | artifact extracted, run under `qemu-aarch64` | passed | emulation, not hardware |
| Windows x86_64 | `install.ps1` | 9/9 passed | updater renames the running exe aside |
| macOS | not executed | — | no macOS runner was assigned; cancelled twice |
| Termux aarch64 | installer selects the Android archive | packaging passed | no runner can execute Termux |

## What the candidate work changed

- The release gate used `update --version`, a flag that had been renamed to
  `--tag`; the gate would have failed on the first real tag.
- The release job is now guarded with `startsWith(github.ref, 'refs/tags/v')`, so
  a manual dispatch can never create a release from a branch.
- Termux had no artifact it could run: the release only shipped a glibc build.
  There is now an `aarch64-linux-android` archive, and both the installer and
  `zenx update` select it on Termux.
- Windows could not replace or delete its own running executable, so update,
  rollback and uninstall were broken there. They now move the file aside and say
  exactly what they did.
- The changelog had an `Unreleased` section holding the release's own changes; it
  is folded into `0.1.0`.

## Remaining risks

1. **macOS is untested at runtime.** Both archives build and checksum, but no
   runner executed them. This is the largest gap.
2. **Termux is unverified on a device.** The archive is Bionic-linked, has the
   right layout, and the installer selects it; nobody has run it on a phone. The
   procedure is [termux-smoke-test.md](termux-smoke-test.md).
3. **crates.io is unpublished**, so `cargo install zenx` does not work. Documented
   as such everywhere.
4. **Executable bits.** Scripts are invoked as `sh install.sh` / `bash scripts/…`
   because the file-API used to push them does not carry modes. A `chmod +x` after
   cloning restores the conventional form.
5. **Windows leaves `zenx.exe.old`** behind after uninstall; Windows cannot delete
   a running program. Documented in the release notes.

## Is v0.1.0 ready?

Ready to tag, with one caveat the maintainer must accept or close first: macOS
runtime testing has not happened. Everything else on the checklist passed —
builds on six targets, 150/147 tests per platform with zero failures, clippy and
rustfmt clean, checksums verified, installer and updater exercised end to end on
Linux and Windows, and uninstall proven to preserve user data.

Recommended before tagging:

1. Run the Termux smoke test on a real device (about ten minutes).
2. Push an empty commit or re-dispatch the candidate workflow when a macOS runner
   is available, and confirm `runtime test macOS` turns green.

Then:

```bash
git tag -a v0.1.0 -m "zenX v0.1.0" && git push origin v0.1.0
```

That push is what creates the release, because the release job only runs on a
`v*` tag. It is irreversible in the sense that a published release is public, so
it is left to you.
