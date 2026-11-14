<#
.SYNOPSIS
  Builds the Afterbattle Windows installer and writes SHA256SUMS.txt next to it.

.DESCRIPTION
  Run by the author from the repository root on a clean checkout of the release commit:

    powershell -ExecutionPolicy Bypass -File scripts\release.ps1

  Steps: checks the working tree is clean, installs locked dependencies, runs the frontend and
  Rust checks (skip with -SkipChecks), builds the unsigned NSIS installer with Tauri and writes
  SHA256SUMS.txt (lowercase SHA-256, two spaces, file name) in the bundle folder. The version
  comes from src-tauri/Cargo.toml, the single source of truth. Nothing is uploaded or tagged.

.PARAMETER SkipChecks
  Skips pnpm lint/test and cargo fmt/clippy/test (for a repeated build of an already checked
  commit).
#>
[CmdletBinding()]
param(
  [switch]$SkipChecks
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Invoke-Step([string]$Name, [scriptblock]$Command) {
  Write-Host "==> $Name" -ForegroundColor Cyan
  & $Command
  if ($LASTEXITCODE -ne 0) { throw "$Name failed (exit code $LASTEXITCODE)" }
}

# The version from Cargo.toml ([package] section).
$cargo = Get-Content -Raw -Path 'src-tauri\Cargo.toml'
$match = [regex]::Match($cargo, '(?ms)^\[package\][^\[]*?^version\s*=\s*"([^"]+)"')
if (-not $match.Success) { throw 'No [package] version in src-tauri/Cargo.toml' }
$version = $match.Groups[1].Value
Write-Host "Afterbattle $version" -ForegroundColor Green

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
Invoke-Step 'tauri build (NSIS)' { pnpm tauri build --bundles nsis }

$bundle = Join-Path $root 'src-tauri\target\release\bundle\nsis'
$installer = Get-ChildItem -Path $bundle -Filter "Afterbattle_${version}_*-setup.exe" |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1
if (-not $installer) { throw "No installer for $version in $bundle" }

$hash = (Get-FileHash -Algorithm SHA256 -Path $installer.FullName).Hash.ToLowerInvariant()
$sums = Join-Path $bundle 'SHA256SUMS.txt'
# UTF-8 without BOM, LF, the format `sha256sum -c` reads.
[System.IO.File]::WriteAllText($sums, "$hash  $($installer.Name)`n", (New-Object System.Text.UTF8Encoding $false))

Write-Host ''
Write-Host "Commit:     $commit"
Write-Host "Installer:  $($installer.FullName)"
Write-Host "Size:       $([math]::Round($installer.Length / 1MB, 1)) MB"
Write-Host "SHA-256:    $hash"
Write-Host "Checksums:  $sums"
