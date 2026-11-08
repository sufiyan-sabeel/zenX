# Installing on Windows

Supported architecture: **x86_64**. Windows ARM64 has no prebuilt binary yet;
build from source instead.

## PowerShell installer

```powershell
irm https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.ps1 | iex
```

It downloads `zenx-x86_64-pc-windows-msvc.zip`, verifies it against
`SHA256SUMS.txt` with `Get-FileHash`, and only then extracts it. A missing or
mismatched checksum stops the install.

Installs to `%LOCALAPPDATA%\zenX\bin\zenx.exe`. Options:

```powershell
.\install.ps1 -Version v0.1.0
.\install.ps1 -Dir C:\tools\zenx
.\install.ps1 -SetPath     # append the directory to your user PATH
.\install.ps1 -Rollback    # restore zenx.exe.old from the previous install
```

`-SetPath` reads the user `Path` variable, checks whether the directory is
already present, and only appends when it is not.

**Status:** the PowerShell script has been written but has not yet been executed
on a Windows machine by the maintainers, because the development environment for
this release had no Windows host. Treat the manual steps below as the verified
path until CI runs the script on a `windows-latest` runner.

## Manual install

1. Download `zenx-x86_64-pc-windows-msvc.zip` from
   [releases](https://github.com/sufiyan-sabeel/zenX/releases/latest).
2. Download `SHA256SUMS.txt` from the same release.
3. Check the hash in PowerShell:

   ```powershell
   Get-FileHash .\zenx-x86_64-pc-windows-msvc.zip -Algorithm SHA256
   ```

   Compare it with the line in `SHA256SUMS.txt`. Only continue if they match.
4. Expand the archive and move `zenx.exe` somewhere permanent, for example
   `%LOCALAPPDATA%\zenX\bin\`.
5. Add that directory to your user `PATH`:

   ```powershell
   [Environment]::SetEnvironmentVariable(
     "Path",
     [Environment]::GetEnvironmentVariable("Path", "User") + ";$env:LOCALAPPDATA\zenX\bin",
     "User")
   ```

6. Open a new terminal and run `zenx --version`.

## From source

Install the Rust toolchain and the MSVC build tools (SQLite is compiled from
source, so a linker and C compiler are required):

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools
git clone https://github.com/sufiyan-sabeel/zenX
cd zenX
cargo build --release
copy target\release\zenx.exe "$env:LOCALAPPDATA\zenX\bin\zenx.exe"
```

## Updating and uninstalling

```powershell
zenx update --check
zenx update
zenx uninstall            # shows the plan
zenx uninstall --yes      # removes zenx.exe only
```

The updater unpacks releases with `tar`, which Windows 10 (1803 and later) and
Windows 11 include as `bsdtar`.

## Notes

- Paths are handled with `\` on Windows; zenX normalises them for display only.
- `zenx doctor --build` works, but a full workspace build on Windows is slow.
- WSL is not required. If you do use WSL, follow the Linux guide inside it.
