<#
.SYNOPSIS
  Builds the public snapshot of Afterbattle: the tree of a ref minus the excluded files, scanned
  for secrets, personal data and game files, committed in a separate repository.

.DESCRIPTION
  Run by the author from the repository root:

    powershell -ExecutionPolicy Bypass -File scripts\make-public-snapshot.ps1

  The snapshot is the tree of -Ref (default v2/integration) without the whole files and folders
  listed in $Exclusions below; no file content is changed. Then:
    1. gitleaks (dir mode) and pattern searches run on the snapshot; any finding that is not in
       $KnownGitleaks stops the script before anything is committed;
    2. new mode (target folder missing): `git init -b main` and the first commit;
       update mode (target folder is already a git repository): the tree is replaced and a new
       commit is added, so later releases keep a history.
  The commit author is read from the logged-in GitHub account (`gh api user`, read only):
  name (or login) and `<id>+<login>@users.noreply.github.com`, set as user.name/user.email in
  the public repository only. No commit without a GitHub noreply address.
  Nothing is pushed or published.

.PARAMETER Target
  The public repository folder (default: ..\afterbattle-public next to this repository).

.PARAMETER Ref
  The ref whose tree is exported (default: v2/integration).

.PARAMETER Gitleaks
  Path to gitleaks.exe; default: `gitleaks` on PATH, else the winget package folder.

.PARAMETER NoCommit
  Builds and scans the snapshot but does not create or change any git repository (dry run).

.PARAMETER AuthorEmail
  Overrides the address read from GitHub; it must still end with @users.noreply.github.com.

.PARAMETER AuthorName
  Overrides the name read from GitHub. With both overrides, gh is not asked.

.PARAMETER GhCommand
  The gh executable (tests pass a substitute).

.PARAMETER ResolveAuthorOnly
  Prints "AUTHOR: name <email>" and stops (used by the tests).
#>
[CmdletBinding()]
param(
  [string]$Target = '',
  [string]$Ref = 'v2/integration',
  [string]$Gitleaks = '',
  [switch]$NoCommit,
  [string]$AuthorEmail = '',
  [string]$AuthorName = '',
  [string]$GhCommand = 'gh',
  [switch]$ResolveAuthorOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------------------------
# Data: what never goes public (whole files and folders; a trailing "/" means a folder, "*"
# matches within one path segment). Kept in sync with docs/PUBLIC_RELEASE_REVIEW.md.
# ---------------------------------------------------------------------------------------------
$Exclusions = @(
  'docs/research/',               # research notes, including path B (encrypted replay part)
  'docs/audit/',                  # internal audit; names commits with personal data
  'docs/status/*.md',             # internal task and status notes (screens/ stays: synthetic)
  'STATUS.md',                    # internal run log with local paths and golden numbers
  'CLAUDE.md',                    # instructions for the coding agent
  'docs/IMPLEMENTATION_PLAN.md',  # outdated alpha plan (Tomato.gg, python.log)
  'src-tauri/tools/',             # archived alpha tool that could modify a real database
  '.github/dependabot.yml',       # dependency updates only in the private repository
  '.claude/'                      # local agent settings (not tracked; listed for safety)
)

# Known gitleaks false positives: game identifiers matched by the generic-api-key rule. Each line
# carries `gitleaks:allow`, so this list does not report itself.
$KnownGitleaks = @(
  @{ Rule = 'generic-api-key'; File = 'src/test/fixtures.ts'; Secret = '90_synthetic_hills' },  # gitleaks:allow
  @{ Rule = 'generic-api-key'; File = 'src/test/fixtures.ts'; Secret = '92_synthetic_hills' },  # gitleaks:allow
  @{ Rule = 'generic-api-key'; File = 'src/app/screens.test.tsx'; Secret = '90_synthetic_hills' },  # gitleaks:allow
  @{ Rule = 'generic-api-key'; File = 'src/app/screens.test.tsx'; Secret = 'synth-sx9_vz_60s_test' },  # gitleaks:allow
  @{ Rule = 'generic-api-key'; File = 'src/lib/presentation.test.ts'; Secret = 'usa-A179_Black_Rock' },  # gitleaks:allow
  @{ Rule = 'generic-api-key'; File = 'src/lib/presentation.test.ts'; Secret = '99_synthetic_map' },  # gitleaks:allow
  @{ Rule = 'generic-api-key'; File = 'src-tauri/src/local_tracker.rs'; Secret = 'france-amx_13' }  # gitleaks:allow
)

# Pattern searches over the snapshot's text files; any hit stops the script.
$Patterns = [ordered]@{
  'account id'       = '(?i)\b(account_?id|accountdbid|playerid|player_id|dbid)\b["'':= ]{1,4}["'']?\d{6,10}\b'
  # Synthetic test paths use the user folder "private" (diagnostics privacy test); any other
  # user name is a finding.
  'Windows user path' = '(?i)\bC:(?>[\\/]+)Users(?>[\\/]+)(?!private[\\/])'
  'AWS key'          = '\bAKIA[0-9A-Z]{16}\b'
  'GitHub token'     = '\bgh[pousr]_[A-Za-z0-9]{30,}|\bgithub_pat_[A-Za-z0-9_]{20,}'
  'Slack token'      = '\bxox[abprs]-[A-Za-z0-9-]{10,}'
  'API secret key'   = '\bsk-[A-Za-z0-9]{32,}'
  'private key'      = '-----BEGIN [A-Z ]*PRIVATE KEY-----'
  'Tomato.gg key'    = '\btmgg_[A-Za-z0-9]{16,}'
  'replay file name' = '\b(?!20200101_0000_)\d{8}_\d{4}_[A-Za-z0-9_.-]*\.wotreplay\b'
}
# The only files that may name the excluded files: this script and its test.
$NameScanExempt = @('scripts/make-public-snapshot.ps1', 'src/publicSnapshot.test.ts')
$GameFileExtensions = @('.dds', '.pkg', '.mo', '.wotreplay')
$BinaryExtensions = @('.png', '.ico', '.icns', '.woff2', '.woff', '.ttf', '.exe', '.jpg', '.gif')
$SkipFiles = @('pnpm-lock.yaml', 'src-tauri/Cargo.lock')

# ---------------------------------------------------------------------------------------------

$root = Split-Path -Parent $PSScriptRoot
if (-not $Target) { $Target = Join-Path (Split-Path -Parent $root) 'afterbattle-public' }
$Target = [System.IO.Path]::GetFullPath($Target)

function Invoke-Git([string[]]$Arguments) {
  $output = & git @Arguments
  if ($LASTEXITCODE -ne 0) { throw "git $($Arguments -join ' ') failed ($LASTEXITCODE)" }
  $output
}

function Test-Excluded([string]$Path) {
  foreach ($rule in $Exclusions) {
    if ($rule.EndsWith('/')) {
      if ($Path.StartsWith($rule)) { return $true }
    } else {
      $regex = '^' + ([regex]::Escape($rule) -replace '\\\*', '[^/]*') + '$'
      if ($Path -match $regex) { return $true }
    }
  }
  return $false
}

function Find-Gitleaks {
  if ($Gitleaks) { return $Gitleaks }
  $command = Get-Command gitleaks -ErrorAction SilentlyContinue
  if ($command) { return $command.Source }
  $package = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Recurse -Filter gitleaks.exe -ErrorAction SilentlyContinue |
    Select-Object -First 1
  if ($package) { return $package.FullName }
  throw 'gitleaks not found (winget install --id Gitleaks.Gitleaks, or pass -Gitleaks)'
}

function Invoke-Gh([string[]]$Arguments) {
  # gh writes progress and errors to stderr; only stdout and the exit code count.
  $ErrorActionPreference = 'Continue'
  try {
    $output = & $GhCommand @Arguments 2>$null
    return @{ Code = $LASTEXITCODE; Output = (@($output) -join "`n").Trim() }
  } catch {
    return @{ Code = 1; Output = '' }
  }
}

# The commit author: the logged-in GitHub account's noreply address and name (read only).
function Resolve-Author {
  $email = $AuthorEmail
  $name = $AuthorName
  if (-not $email -or -not $name) {
    if ((Invoke-Gh @('auth', 'status')).Code -ne 0) {
      throw 'gh is not logged in (run: gh auth login); refusing to commit'
    }
    $read = Invoke-Gh @('api', 'user')
    $user = $null
    if ($read.Code -eq 0 -and $read.Output) {
      try { $user = ConvertFrom-Json $read.Output } catch { $user = $null }
    }
    $hasId = $user -and ($user.PSObject.Properties.Name -contains 'id') -and $user.id
    $hasLogin = $user -and ($user.PSObject.Properties.Name -contains 'login') -and $user.login
    if (-not $hasId -or -not $hasLogin) {
      throw 'reading the GitHub account failed (gh api user); refusing to commit'
    }
    if (-not $email) { $email = "$($user.id)+$($user.login)@users.noreply.github.com" }
    if (-not $name) {
      $hasName = ($user.PSObject.Properties.Name -contains 'name') -and $user.name
      $name = if ($hasName) { [string]$user.name } else { [string]$user.login }
    }
  }
  if ($email -notmatch '^[^@\s]+@users\.noreply\.github\.com$') {
    throw "the author address must be a GitHub noreply address (...@users.noreply.github.com), got '$email'; refusing to commit"
  }
  if (-not $name.Trim()) { throw 'the author name is empty; refusing to commit' }
  return @{ Name = $name.Trim(); Email = $email }
}

if ($ResolveAuthorOnly) {
  $author = Resolve-Author
  Write-Output "AUTHOR: $($author.Name) <$($author.Email)>"
  return
}
# Decided before any work, so a missing login stops the run early.
$author = if ($NoCommit) { $null } else { Resolve-Author }

Set-Location $root
$commit = (Invoke-Git @('rev-parse', '--short', "$Ref^{commit}")).Trim()
$files = @(Invoke-Git @('ls-tree', '-r', '--name-only', $Ref))
$excluded = @($files | Where-Object { Test-Excluded $_ })
$kept = @($files | Where-Object { -not (Test-Excluded $_) })
$cargo = (Invoke-Git @('show', "${Ref}:src-tauri/Cargo.toml")) -join "`n"
$version = [regex]::Match($cargo, '(?ms)^\[package\][^\[]*?^version\s*=\s*"([^"]+)"').Groups[1].Value
Write-Host "Afterbattle $version, $Ref at ${commit}: $($kept.Count) files kept, $($excluded.Count) excluded" -ForegroundColor Green

# Mode.
$update = Test-Path (Join-Path $Target '.git')
if ((Test-Path $Target) -and -not $update) {
  if (Get-ChildItem -Force $Target) { throw "$Target exists and is not a git repository; remove it or choose another -Target" }
}
if ($NoCommit -and $update) { throw 'Dry run (-NoCommit) needs a new -Target folder, not an existing repository' }
New-Item -ItemType Directory -Force -Path $Target | Out-Null

# Replace the tree (the .git folder stays in update mode).
Get-ChildItem -Force $Target | Where-Object { $_.Name -ne '.git' } | Remove-Item -Recurse -Force
$tar = Join-Path ([System.IO.Path]::GetTempPath()) "afterbattle-snapshot-$commit.tar"
Invoke-Git @('archive', '--format=tar', "--output=$tar", $Ref) | Out-Null
& tar -xf $tar -C $Target
if ($LASTEXITCODE -ne 0) { throw 'tar failed' }
Remove-Item $tar -Force
foreach ($path in $excluded) {
  $full = Join-Path $Target $path
  if (Test-Path $full) { Remove-Item -Force $full }
}
Get-ChildItem -Recurse -Directory -Force $Target |
  Where-Object { $_.FullName -notmatch '[\\/]\.git([\\/]|$)' } |
  Sort-Object { $_.FullName.Length } -Descending |
  Where-Object { -not (Get-ChildItem -Force $_.FullName) } |
  Remove-Item -Force
foreach ($rule in $Exclusions) {
  if ($rule.EndsWith('/') -and (Test-Path (Join-Path $Target $rule))) { throw "excluded folder still present: $rule" }
}

# Scans.
$problems = New-Object System.Collections.Generic.List[string]
$relative = { param($full) $full.Substring($Target.Length + 1).Replace('\', '/') }
$snapshotFiles = Get-ChildItem -Recurse -File -Force $Target |
  Where-Object { $_.FullName -notmatch '[\\/]\.git[\\/]' }

foreach ($file in $snapshotFiles) {
  if ($GameFileExtensions -contains $file.Extension.ToLowerInvariant()) {
    $problems.Add("game file: $(& $relative $file.FullName)")
  }
}

$textFiles = $snapshotFiles | Where-Object {
  ($BinaryExtensions -notcontains $_.Extension.ToLowerInvariant()) -and
  ($SkipFiles -notcontains (& $relative $_.FullName))
}
foreach ($name in $Patterns.Keys) {
  $hits = $textFiles | Select-String -Pattern $Patterns[$name] -AllMatches -ErrorAction SilentlyContinue
  foreach ($hit in $hits) {
    $problems.Add("$name`: $(& $relative $hit.Path):$($hit.LineNumber)")
  }
}

# No public file may name an excluded file: its path, its file name, or an excluded folder.
$excludedNames = New-Object System.Collections.Generic.List[string]
foreach ($path in $excluded) {
  $excludedNames.Add($path)
  $excludedNames.Add(($path -split '/')[-1])
}
foreach ($rule in $Exclusions) {
  if ($rule.EndsWith('/')) { $excludedNames.Add($rule.TrimEnd('/')) }
}
$nameAlternatives = ($excludedNames | Sort-Object -Unique | ForEach-Object { [regex]::Escape($_) }) -join '|'
$namePattern = "(?i)(?<![A-Za-z0-9_.-])($nameAlternatives)(?![A-Za-z0-9_-])"
$nameFiles = $textFiles | Where-Object { $NameScanExempt -notcontains (& $relative $_.FullName) }
$nameLabel = 'excluded name'
$nameHits = $nameFiles | Select-String -Pattern $namePattern -AllMatches -ErrorAction SilentlyContinue
foreach ($hit in $nameHits) {
  $found = ($hit.Matches | ForEach-Object { $_.Value } | Sort-Object -Unique) -join ', '
  $problems.Add("$nameLabel ($found): $(& $relative $hit.Path):$($hit.LineNumber)")
}

$gitleaksExe = Find-Gitleaks
$report = Join-Path ([System.IO.Path]::GetTempPath()) "afterbattle-gitleaks-$commit.json"
& $gitleaksExe dir $Target --report-format json --report-path $report --no-banner --log-level error | Out-Null
if (-not (Test-Path $report)) { throw 'gitleaks wrote no report' }
# Windows PowerShell 5.1 returns a JSON array as one object; foreach unrolls it.
$parsed = ConvertFrom-Json (Get-Content -Raw $report)
$findings = @(foreach ($item in $parsed) { $item })
Remove-Item $report -Force
$known = 0
foreach ($finding in $findings) {
  $file = $finding.File.Replace('\', '/')
  $prefix = $Target.Replace('\', '/') + '/'
  if ($file.StartsWith($prefix)) { $file = $file.Substring($prefix.Length) }
  $match = $KnownGitleaks | Where-Object {
    $_.Rule -eq $finding.RuleID -and $_.File -eq $file -and $_.Secret -eq $finding.Secret
  }
  if ($match) { $known++ } else { $problems.Add("gitleaks $($finding.RuleID): ${file}:$($finding.StartLine)") }
}
Write-Host "gitleaks: $($findings.Count) findings, $known known false positives"

if ($problems.Count -gt 0) {
  $problems | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
  throw "$($problems.Count) new scan finding(s); nothing was committed. Fix the source or extend the data above after review."
}
Write-Host 'Scans: no new findings.' -ForegroundColor Green

if ($NoCommit) {
  Write-Host "Dry run: snapshot in $Target, no git repository created."
  Write-Host "Excluded:`n  $($excluded -join "`n  ")"
  return
}

# Commit: only the exported files (explicit list) and deletions of tracked files.
Push-Location $Target
try {
  if (-not $update) { Invoke-Git @('init', '-q', '-b', 'main') | Out-Null }
  # The author only in this repository's own config (never the global one).
  Invoke-Git @('config', 'user.name', $author.Name) | Out-Null
  Invoke-Git @('config', 'user.email', $author.Email) | Out-Null
  $list = Join-Path ([System.IO.Path]::GetTempPath()) "afterbattle-snapshot-files-$commit.txt"
  [System.IO.File]::WriteAllLines($list, [string[]]$kept)
  Invoke-Git @('add', "--pathspec-from-file=$list") | Out-Null
  if ($update) { Invoke-Git @('add', '--update') | Out-Null }
  Remove-Item $list -Force
  $untracked = @(Invoke-Git @('status', '--porcelain', '--untracked-files=all') | Where-Object { $_.StartsWith('??') })
  if ($untracked.Count -gt 0) { throw "unexpected untracked files: $($untracked -join ', ')" }
  $message = "Afterbattle $version`n`nPublic snapshot of the private repository ($Ref at $commit) without the excluded files listed in scripts/make-public-snapshot.ps1."
  Invoke-Git @('commit', '-q', '-m', $message) | Out-Null
  $head = (Invoke-Git @('log', '--oneline', '-1')).Trim()
  Write-Host "Committed in ${Target}: $head" -ForegroundColor Green
} finally {
  Pop-Location
}
