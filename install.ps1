<#
.SYNOPSIS
  Install zenX on Windows.

.DESCRIPTION
  Downloads the prebuilt archive for this machine, verifies it against the
  release's SHA256SUMS.txt, and only then installs it. A missing or mismatched
  checksum stops the installation before anything is extracted.

.EXAMPLE
  irm https://raw.githubusercontent.com/sufiyan-sabeel/zenX/main/install.ps1 | iex

.EXAMPLE
  .\install.ps1 -Version v0.1.0 -SetPath
#>
[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$Dir = "",
    [switch]$SetPath,
    [switch]$Rollback
)

$ErrorActionPreference = "Stop"
$Repo = "sufiyan-sabeel/zenX"

$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
    "AMD64" { $target = "x86_64-pc-windows-msvc" }
    "ARM64" { throw "No prebuilt Windows ARM64 release exists yet. Build from source: cargo install --git https://github.com/$Repo zenx" }
    default { throw "Unsupported architecture: $arch" }
}

$asset = "zenx-$target.zip"

if ($env:ZENX_RELEASE_BASE_URL) {
    $base = $env:ZENX_RELEASE_BASE_URL.TrimEnd('/')
    if ($base -notmatch '^https?://' -and -not (Test-Path $base)) {
        throw "ZENX_RELEASE_BASE_URL must be an http(s) URL or an existing directory: $base"
    }
} elseif ($Version) {
    $base = "https://github.com/$Repo/releases/download/$Version"
} else {
    $base = "https://github.com/$Repo/releases/latest/download"
}

if (-not $Dir) { $Dir = Join-Path $env:LOCALAPPDATA "zenX\bin" }
New-Item -ItemType Directory -Force -Path $Dir | Out-Null

if ($Rollback) {
    $dest = Join-Path $Dir "zenx.exe"
    if (-not (Test-Path "$dest.old")) {
        throw "No previous binary at $dest.old, nothing to roll back"
    }
    Move-Item -Force "$dest.old" $dest
    Write-Host "zenx installer: rolled back to $(& $dest --version) at $dest"
    exit 0
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("zenx-install-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
    $archive = Join-Path $tmp $asset
    $sums = Join-Path $tmp "SHA256SUMS.txt"

    function Get-ReleaseFile {
        param([string]$Name, [string]$Destination, [switch]$Required)
        $source = if ($base -match '^https?://') { "$base/$Name" } else { Join-Path $base $Name }
        try {
            if ($source -match '^https?://') {
                Invoke-WebRequest -Uri $source -OutFile $Destination -UseBasicParsing
            } else {
                if (-not (Test-Path $source)) { throw "not found: $source" }
                Copy-Item -LiteralPath $source -Destination $Destination -Force
            }
        } catch {
            if ($Required) { throw }
            return $false
        }
        return $true
    }

    Write-Host "zenx installer: platform $target"
    Write-Host "zenx installer: downloading $asset"
    Get-ReleaseFile -Name $asset -Destination $archive -Required | Out-Null

    if (-not (Get-ReleaseFile -Name "SHA256SUMS.txt" -Destination $sums)) {
        throw "Refusing to install: this release publishes no SHA256SUMS.txt. An unverifiable download is never installed."
    }

    $match = Select-String -Path $sums -Pattern ([regex]::Escape($asset) + '\s*$') | Select-Object -First 1
    if (-not $match) {
        throw "Refusing to install: SHA256SUMS.txt has no entry for $asset"
    }
    $expected = ($match.Line -split '\s+')[0].ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 -Path $archive).Hash.ToLower()
    if ($expected -ne $actual) {
        throw "Checksum mismatch, refusing to install. Published: $expected Downloaded: $actual"
    }
    Write-Host "zenx installer: checksum verified ($actual)"

    Expand-Archive -LiteralPath $archive -DestinationPath $tmp -Force
    $found = Get-ChildItem -Path $tmp -Recurse -Filter "zenx.exe" | Select-Object -First 1
    if (-not $found) { throw "The archive did not contain zenx.exe" }

    $dest = Join-Path $Dir "zenx.exe"
    if (Test-Path $dest) { Copy-Item $dest "$dest.old" -Force }
    Copy-Item $found.FullName $dest -Force

    $installedVersion = & $dest --version
    if (-not $installedVersion) { throw "The installed binary did not run" }
    Write-Host "zenx installer: installed $installedVersion to $dest"

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $entries = @()
    if ($userPath) { $entries = $userPath.Split(";") | Where-Object { $_ } }
    if ($entries -notcontains $Dir) {
        if ($SetPath) {
            $newPath = (@($entries) + $Dir) -join ";"
            [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
            Write-Host "zenx installer: added $Dir to your user PATH (open a new terminal)"
        } else {
            Write-Host "zenx installer: $Dir is not on PATH yet"
            Write-Host "  Add it, or rerun with -SetPath"
        }
    }

    Write-Host ""
    Write-Host "Next: zenx init; zenx status"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
