<#
.SYNOPSIS
  Builds the Afterbattle Windows installer with signed update artifacts, latest.json and
  SHA256SUMS.txt.

.DESCRIPTION
  Run by the author from the repository root on a clean checkout of the release commit:

    $env:TAURI_SIGNING_PRIVATE_KEY = 'C:\path\outside\the\repo\afterbattle.key'  # path or content
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '...'                               # if the key has one
    powershell -ExecutionPolicy Bypass -File scripts\release.ps1

  Steps: refuses to start without TAURI_SIGNING_PRIVATE_KEY (spec 32 section 5.2: no release
  without update signatures), checks the working tree is clean, installs locked dependencies,
  runs the frontend and Rust checks (skip with -SkipChecks), builds the NSIS installer with
  Tauri and the update artifacts turned on through --config (tauri.conf.json keeps them off, so
  the public CI builds without the key), then writes in the bundle folder:

    latest.json      the update manifest for the endpoint in tauri.conf.json (version, notes,
                     date, the installer's signature and its download address on GitHub)
    SHA256SUMS.txt   lowercase SHA-256, two spaces, file name

  The version comes from src-tauri/Cargo.toml, the single source of truth. The notes are the
  CHANGELOG section of that version (or -NotesFile). Nothing is uploaded or tagged; the script
  never prints or copies the private key.

.PARAMETER SkipChecks
  Skips pnpm lint/test and cargo fmt/clippy/test (for a repeated build of an already checked
  commit).

.PARAMETER NotesFile
  A UTF-8 file with the release notes for latest.json instead of the CHANGELOG section.
#>
[CmdletBinding()]
param(
  [switch]$SkipChecks,
  [string]$NotesFile
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# The repository the update endpoint points to (tauri.conf.json, plugins.updater.endpoints).
$releasesBase = 'https://github.com/dataflorist02/afterbattle-beta/releases/download'

function Invoke-Step([string]$Name, [scriptblock]$Command) {
  Write-Host "==> $Name" -ForegroundColor Cyan
  & $Command
  if ($LASTEXITCODE -ne 0) { throw "$Name failed (exit code $LASTEXITCODE)" }
}

# No key, no build: an installer without update signatures cannot be offered by the updater.
if ([string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY)) {
  throw 'TAURI_SIGNING_PRIVATE_KEY is not set (path to the private update key or its content, kept outside the repository). Refusing to build a release without update artifacts.'
}
if ([string]::IsNullOrEmpty($env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD)) {
  Write-Host 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD is not set; fine only for a key without a password.' -ForegroundColor Yellow
}

# The version from Cargo.toml ([package] section).
$cargo = Get-Content -Raw -Path 'src-tauri\Cargo.toml'
$match = [regex]::Match($cargo, '(?ms)^\[package\][^\[]*?^version\s*=\s*"([^"]+)"')
if (-not $match.Success) { throw 'No [package] version in src-tauri/Cargo.toml' }
$version = $match.Groups[1].Value
Write-Host "Afterbattle $version" -ForegroundColor Green

# Release notes for latest.json: the CHANGELOG section "## <version>" up to the next "## ".
if ($NotesFile) {
  $notes = (Get-Content -Raw -Encoding UTF8 -Path $NotesFile).Trim()
} else {
  $lines = Get-Content -Encoding UTF8 -Path 'CHANGELOG.md'
  $start = [Array]::IndexOf($lines, "## $version")
  if ($start -lt 0) { throw "No '## $version' section in CHANGELOG.md (or pass -NotesFile)" }
  $section = New-Object System.Collections.Generic.List[string]
  for ($i = $start + 1; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^## ') { break }
    $section.Add($lines[$i])
  }
  $notes = ($section -join "`n").Trim()
}
if (-not $notes) { throw 'The release notes are empty' }

$dirty = git status --porcelain
if ($dirty) {
  throw "The working tree is not clean; commit or remove these first:`n$dirty"
}
$commit = (git rev-parse --short HEAD).Trim()

Invoke-Step 'pnpm install' { pnpm install --frozen-lockfile }
if (-not $SkipChecks) {
  Invoke-Step 'pnpm lint:setup' { pnpm lint:setup }
  Invoke-Step 'pnpm lint' { pnpm lint }
  Invoke-Step 'pnpm test' { pnpm test }
  Invoke-Step 'cargo fmt' { cargo fmt --manifest-path src-tauri/Cargo.toml -- --check }
  Invoke-Step 'cargo clippy' { cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings }
  Invoke-Step 'cargo test' { cargo test --manifest-path src-tauri/Cargo.toml }
}

# Update artifacts on for this build only, from a temporary config file (quotes in a JSON
# argument do not survive Windows PowerShell's native argument passing).
$updaterConfig = Join-Path ([System.IO.Path]::GetTempPath()) "afterbattle-updater-$PID.json"
[System.IO.File]::WriteAllText($updaterConfig, '{"bundle":{"createUpdaterArtifacts":true}}', (New-Object System.Text.UTF8Encoding $false))
try {
  Invoke-Step 'tauri build (NSIS, update artifacts)' { pnpm tauri build --bundles nsis --config $updaterConfig }
} finally {
  Remove-Item -Force -ErrorAction SilentlyContinue $updaterConfig
}

$bundle = Join-Path $root 'src-tauri\target\release\bundle\nsis'
$installer = Get-ChildItem -Path $bundle -Filter "Afterbattle_${version}_*-setup.exe" |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1
if (-not $installer) { throw "No installer for $version in $bundle" }
$signatureFile = "$($installer.FullName).sig"
if (-not (Test-Path $signatureFile)) { throw "No update signature $signatureFile (was the key accepted?)" }
$signature = (Get-Content -Raw -Path $signatureFile).Trim()

# latest.json (Tauri v2 updater, static JSON): the installer itself is the update package.
$manifest = [ordered]@{
  version   = $version
  notes     = $notes
  pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = [ordered]@{
    'windows-x86_64' = [ordered]@{
      signature = $signature
      url       = "$releasesBase/v$version/$($installer.Name)"
    }
  }
}
$latest = Join-Path $bundle 'latest.json'
[System.IO.File]::WriteAllText($latest, ($manifest | ConvertTo-Json -Depth 4), (New-Object System.Text.UTF8Encoding $false))

$hash = (Get-FileHash -Algorithm SHA256 -Path $installer.FullName).Hash.ToLowerInvariant()
$sums = Join-Path $bundle 'SHA256SUMS.txt'
# UTF-8 without BOM, LF, the format `sha256sum -c` reads.
[System.IO.File]::WriteAllText($sums, "$hash  $($installer.Name)`n", (New-Object System.Text.UTF8Encoding $false))

Write-Host ''
Write-Host "Commit:     $commit"
Write-Host "Installer:  $($installer.FullName)"
Write-Host "Size:       $([math]::Round($installer.Length / 1MB, 1)) MB"
Write-Host "SHA-256:    $hash"
Write-Host "Signature:  $signatureFile"
Write-Host "Manifest:   $latest"
Write-Host "Checksums:  $sums"
Write-Host 'Upload the installer, latest.json and SHA256SUMS.txt to the release (tag v<version>).'
