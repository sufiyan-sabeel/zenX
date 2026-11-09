# Termux smoke test

This is the procedure for validating zenX on a real Android device. It exists
because no GitHub-hosted runner can execute a Termux binary: the Android artifact
is built and inspected in CI, but only a device proves it runs.

Run these in Termux. Every command prints what you should see.

## 0. Use the right Termux

Install Termux from **F-Droid** or the **GitHub releases**. The Play Store build
is outdated and 32-bit, and no aarch64 artifact will run there.

Check:

```bash
uname -m          # expect: aarch64
echo "$PREFIX"    # expect: /data/data/com.termux/files/usr
```

If `uname -m` says `armv7l`, this procedure does not apply: there is no 32-bit
build, and the installer will tell you so.

## 1. Install the toolchain and build zenX

This is the route that works today, before any release is published:

```bash
pkg update
pkg install -y rust clang git
cargo install --git https://github.com/sufiyan-sabeel/zenX zenx
```

`clang` is required: SQLite is compiled from source by `rusqlite`'s bundled
feature, which is exactly what makes zenX work on Android without a system
SQLite.

Expect the build to take several minutes on a phone. Then:

```bash
zenx --version    # expect: zenx 0.1.0
```

If `zenx` is not found, cargo installed it to `~/.cargo/bin`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
grep -q 'cargo/bin' ~/.bashrc || echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
```

## 2. Once `v0.1.0` is released, the prebuilt path

```bash
pkg install -y curl tar
curl -fsSL https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.sh | sh
```

Expect output ending with `installed zenx 0.1.0 to /data/data/com.termux/files/usr/bin/zenx`.
The installer detects Termux, selects the `aarch64-linux-android` archive, and
verifies it against `SHA256SUMS.txt` before unpacking. A missing or mismatched
checksum stops it.

If it instead prints `no prebuilt Termux build for ...`, the device is not
aarch64 — use the Cargo route above.

## 3. Environment report

```bash
zenx mobile diagnose
```

Check that it says:

- `termux yes` with a version,
- `android yes` with a release,
- `aarch64` as the architecture,
- `git` and `rustc` as found (if you built from source).

It must not ask for root, and it must not run `pkg` for you.

## 4. Core commands

```bash
zenx init                 # creates ~/.zenx, config.toml and zenx.db
zenx status               # shows the home, schema version and module status
zenx doctor .             # repository health; run it inside a git clone if you have one
```

`zenx status` should print `initialised yes` and a database schema version of 1
or higher.

## 5. Sessions and skills

```bash
printf '%s\n' '{"kind":"tool_call","name":"read_file","duration_ms":12}' \
  | zenx trace record --agent termux-test --label smoke
zenx trace sessions
zenx trace inspect "$(zenx trace sessions --output json | grep -o 'sess_[0-9a-f]*' | head -n1)"
```

Expect one session with one event and a timeline row.

## 6. Backup and restore

```bash
zenx mobile backup
zenx mobile restore ~/.zenx/backups/zenx-backup-* --overwrite
```

The backup is a plain directory: `config.toml`, `zenx.db`, `skills/`,
`manifest.json`. Restore refuses to overwrite without `--overwrite`.

## 7. Update behaviour on a Cargo install

```bash
zenx update --check
```

Expect: `zenX was installed with cargo; update it with cargo install zenx --force ...`.
The updater must not touch a binary that cargo owns.

## 8. Uninstall safety

```bash
zenx uninstall            # prints the plan, changes nothing
zenx uninstall --yes      # removes the binary, keeps ~/.zenx
ls ~/.zenx                # still there
```

## Report back

If any step differs from the expectation above, send the command, the output,
and `zenx --version`. That is enough to reproduce it; the Android build is
byte-identical in CI, so the difference is usually an environment detail.
