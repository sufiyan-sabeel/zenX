# Supported platforms

Three different claims get confused in project documentation, so this page keeps
them apart:

- **Built** — CI produces an archive for this target from the release commit.
- **Runtime-tested** — CI installed that archive with the shipped installer and
  executed it: `--version`, `--help`, `doctor`, update check, dry-run update,
  update, rollback, and uninstall that preserves data.
- **Device-tested** — a human ran it on real hardware of that kind.

| Platform | Architecture | Artifact | Built | Runtime-tested | Device-tested |
|----------|--------------|----------|:-----:|:--------------:|:-------------:|
| Linux | x86_64 | `zenx-x86_64-unknown-linux-gnu.tar.gz` | yes | yes (ubuntu-latest) | yes (x86_64 Linux host) |
| Linux | aarch64 | `zenx-aarch64-unknown-linux-gnu.tar.gz` | yes | yes (qemu-aarch64 emulation) | no |
| macOS | Apple Silicon | `zenx-aarch64-apple-darwin.tar.gz` | yes | pending (see below) | no |
| macOS | Intel | `zenx-x86_64-apple-darwin.tar.gz` | yes (cross-compiled) | no | no |
| Windows | x86_64 | `zenx-x86_64-pc-windows-msvc.zip` | yes | yes (windows-latest) | no |
| Android / Termux | aarch64 | `zenx-aarch64-linux-android.tar.gz` | yes | packaging only | no |
| Windows | ARM64 | — | no | no | no |
| Linux / Termux | 32-bit (armv7, i686) | — | no | no | no |

## Notes per platform

**Linux x86_64.** The reference target. The installer was run from the release
archive, and all nine checks passed on the installed binary.

**Linux aarch64.** Built natively on an x86_64 runner with
`gcc-aarch64-linux-gnu`, then executed under `qemu-aarch64`. The checks ran, but
emulation is not hardware: no aarch64 Linux machine has run it.

**macOS.** Both architectures are built and checksummed. The runtime job needs a
GitHub-hosted macOS runner, which was not available during the candidate runs, so
this row is honestly marked pending rather than passed. The steps are already in
the workflow and will run as soon as a runner is assigned.

**Windows x86_64.** Fully exercised, including the parts that are genuinely
different on Windows: the updater renames a running executable aside instead of
overwriting it, rollback parks it, and uninstall renames rather than claiming a
deletion it cannot perform. Note that a leftover `zenx.exe.old` may remain after
uninstalling, because Windows will not delete a running program.

**Android / Termux aarch64.** Termux uses Bionic, not glibc, so it gets its own
`aarch64-linux-android` artifact. CI proves the archive layout, that the binary
has no glibc dependency, and that the installer selects the Android archive when
it detects Termux. It cannot execute it: no GitHub-hosted runner runs Termux. The
procedure for a real device is [termux-smoke-test.md](termux-smoke-test.md).

**Windows ARM64.** No prebuilt archive. Build from source with
`cargo install --git https://github.com/sufiyan-sabeel/zenX zenx`.

**32-bit targets.** Not supported. `zenx` needs a 64-bit toolchain, and the
installer stops with an explicit message rather than installing something that
cannot run.

## What "supported" means here

A platform is listed as supported when an archive is published for it and the
installation path (installer or Cargo) is documented. Runtime-tested and
device-tested are separate columns because a build that compiles is not the same
as a binary that runs, and neither is the same as a binary a person has used on
that hardware.
