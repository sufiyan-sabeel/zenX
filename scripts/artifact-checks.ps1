<#
.SYNOPSIS
  Exercise an installed zenX binary on Windows.

.DESCRIPTION
  Mirrors scripts/artifact-checks.sh: version, help, doctor, update detection,
  dry-run, install, rollback, and uninstall that preserves user data.
  Prints PASS/FAIL per check and exits non-zero if any failed.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Binary,
    [Parameter(Mandatory = $true)][string]$Dist
)

$ErrorActionPreference = "Continue"
$failed = 0
function Pass($name) { Write-Host "PASS  $name" }
function Fail($name) { Write-Host "FAIL  $name"; $script:failed++ }

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("zenx-checks-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $work | Out-Null
$release = Join-Path $work "release"
New-Item -ItemType Directory -Force -Path $release | Out-Null
Get-ChildItem -Path $Dist -Filter "*.zip" | Copy-Item -Destination $release -Force
Copy-Item (Join-Path $Dist "SHA256SUMS.txt") -Destination $release -Force

$env:ZENX_HOME = Join-Path $work "home"
& $Binary init | Out-Null

$version = & $Binary --version
if ($version -match '^zenx \d') { Pass "--version ($version)" } else { Fail "--version returned '$version'" }

$help = (& $Binary --help) -join "`n"
$missing = @()
foreach ($command in @("init", "status", "trace", "skills", "doctor", "mobile", "bench", "contract", "docs", "update", "uninstall")) {
    if ($help -notmatch "\s$command\b") { $missing += $command }
}
if ($missing.Count -eq 0) { Pass "--help lists every command" } else { Fail "--help is missing: $($missing -join ', ')" }

$doctor = (& $Binary doctor . --output json) -join "`n"
if ($doctor -match '"checks"') { Pass "doctor emitted a JSON report" } else { Fail "doctor did not emit JSON" }

$env:ZENX_RELEASE_BASE_URL = $release
$check = (& $Binary update --check --tag v9.9.9 --output json) -join "`n"
if ($check -match '"action": "available"') { Pass "update --check reports an available release" } else { Fail "update --check: $check" }

$before = (Get-FileHash -Algorithm SHA256 -Path $Binary).Hash
$dry = (& $Binary update --dry-run --tag v9.9.9 --output json) -join "`n"
$after = (Get-FileHash -Algorithm SHA256 -Path $Binary).Hash
if ($dry -match '"verified": true' -and $before -eq $after) {
    Pass "update --dry-run verified the archive and changed nothing"
} else {
    Fail "update --dry-run: $dry"
}

$install = (& $Binary update --tag v9.9.9 --output json) -join "`n"
if ($install -match '"action": "installed"' -and (Test-Path "$Binary.old")) {
    Pass "update installed and kept a backup"
} else {
    Fail "update: $install"
}

& $Binary update --rollback | Out-Null
if (-not (Test-Path "$Binary.old")) { Pass "update --rollback restored the previous binary" } else { Fail "update --rollback" }

$plan = (& $Binary uninstall --output json) -join "`n"
$db = Join-Path $env:ZENX_HOME "zenx.db"
if ((Test-Path $Binary) -and (Test-Path $db) -and $plan -match '"binary"') {
    Pass "uninstall only reported its plan"
} else {
    Fail "uninstall plan: $plan"
}

& $Binary uninstall --yes | Out-Null
if (-not (Test-Path $Binary) -and (Test-Path $db)) {
    Pass "uninstall removed the binary and kept user data"
} else {
    Fail "uninstall removed the wrong things"
}

Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
Write-Host ""
Write-Host "$failed check(s) failed"
exit $failed
