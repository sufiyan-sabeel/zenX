# Contributing to zenX

Thanks for wanting to help. This project has a narrow definition of "done", and
it is worth reading before you open a pull request.

## Ground rules

1. **Working code only.** A module or command must do what its help text says.
   Stubs, fake success messages and placeholder flags are rejected.
2. **Tests come with the change.** Behaviour that touches the filesystem, the
   database or a subprocess needs a test that would fail without your change.
3. **Never weaken a check to get green.** If a test is wrong, fix the test with
   a reason in the commit message.
4. **No new dependency without a paragraph.** Explain what it does that the
   standard library or an existing dependency cannot, and confirm it builds on
   Android/Termux without a system library that is not available there.
5. **Security paths need hostile tests.** Anything that resolves a user path,
   extracts an archive or writes redacted data must have a test for the attack,
   not only the happy path.

## Getting set up

```bash
git clone https://github.com/sufiyan-sabeel/zenX
cd zenX
cargo build
./scripts/dev-check.sh
```

`dev-check.sh` runs exactly what CI runs: `cargo fmt --check`, `clippy -D
warnings`, the full test suite, and a check that every `zenx` subcommand named
in the documentation exists.

## Adding a module

1. Create `crates/zenx-<name>` and add it to the workspace members and
   `[workspace.dependencies]`.
2. Depend on `zenx-core` (and `zenx-storage`/`zenx-security` if needed) — never
   on the CLI crate.
3. Register it in `zenx_core::module::MODULES` with an honest
   `ModuleStatus`.
4. Add the command to `crates/zenx-cli/src/commands/`.
5. Document it in `docs/CLI.md` and `docs/MODULES.md`.
6. Add unit tests in the crate and, if it has a command, an end-to-end test in
   `crates/zenx-cli/tests/cli.rs`.

## Adding an agent adapter

Adapters are declared in `zenx-trace` (transcripts) and can be added for skills
directories in `zenx-skills`. An adapter is only merged when it:

- implements the trait in the module,
- parses a real sample transcript committed under `examples/`,
- has a unit test for the sample and a test for malformed input,
- is added to the registry in the same change,
- is described accurately in `docs/MODULES.md`.

"Planned" adapters must not appear in the registry. `zenx trace adapters` prints
what exists, and that list is the contract.

## Commit and PR style

- One logical change per pull request.
- Say what breaks and what the user sees differently.
- Include the commands you ran to verify, with their output.

## Code of conduct

Be decent. Disagree with the code, not the person. Harassment, and content that
demeans people for their background, are not welcome here.
