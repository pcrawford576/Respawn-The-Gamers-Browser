# Respawn release build
#   1. raises the version number (0.1.0 -> 0.1.1; use -Bump minor or -Bump major for bigger steps)
#   2. turns the "Unreleased" notes in ui\changelog.md into this version's notes
#      (asks you for notes if there are none)
#   3. builds the installer
# If the build fails, the version and notes are put back as they were.
#
# Run from the respawn folder:
#   powershell -ExecutionPolicy Bypass -File .\release.ps1
#
# Add -Publish to also commit, tag (v1.2.3) and push to GitHub. GitHub then builds
# the installer, gets it code signed and posts it on the Releases page.
#   powershell -ExecutionPolicy Bypass -File .\release.ps1 -Publish

param(
    [ValidateSet('patch', 'minor', 'major')]
    [string]$Bump = 'patch',
    [switch]$Publish
)
$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding $false

$root  = $PSScriptRoot
$conf  = Join-Path $root 'src-tauri\tauri.conf.json'
$cargo = Join-Path $root 'src-tauri\Cargo.toml'
$log   = Join-Path $root 'ui\changelog.md'

# Runs git without PowerShell treating its normal progress messages as errors.
function Invoke-Git {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & git @args 2>&1 | ForEach-Object { Write-Host "  $_" } }
    finally { $ErrorActionPreference = $prev }
    return $LASTEXITCODE
}

# ---- 0. publishing needs git and a GitHub remote; check before touching anything
$remote = $null
if ($Publish) {
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        throw 'Git is not installed. Get it from https://git-scm.com/download/win, then reopen PowerShell.'
    }
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { $remote = (& git -C $root remote get-url origin 2>$null) }
    finally { $ErrorActionPreference = $prev }
    if ($LASTEXITCODE -ne 0 -or -not $remote) {
        throw 'This folder is not connected to GitHub yet. See docs\signing-setup.md, step 2.'
    }
}

$confText  = [IO.File]::ReadAllText($conf, $utf8)
$cargoText = [IO.File]::ReadAllText($cargo, $utf8)
$logText   = [IO.File]::ReadAllText($log, $utf8)

# ---- 1. next version
$m = [regex]::Match($confText, '"version"\s*:\s*"(\d+)\.(\d+)\.(\d+)"')
if (-not $m.Success) { throw 'Could not find "version" in tauri.conf.json' }
$major = [int]$m.Groups[1].Value; $minor = [int]$m.Groups[2].Value; $patch = [int]$m.Groups[3].Value
$old = "$major.$minor.$patch"
switch ($Bump) {
    'major' { $major++; $minor = 0; $patch = 0 }
    'minor' { $minor++; $patch = 0 }
    default { $patch++ }
}
$new = "$major.$minor.$patch"
Write-Host "Respawn $old -> $new" -ForegroundColor Green

# ---- 2. release notes from the "Unreleased" section
$section = [regex]::Match($logText, '(?ms)^## Unreleased[ \t]*\r?\n(.*?)(?=^## |\z)')
if (-not $section.Success) { throw 'ui\changelog.md needs a "## Unreleased" section' }
$notes = $section.Groups[1].Value.Trim()
if (-not $notes) {
    Write-Host "No notes under 'Unreleased' in ui\changelog.md." -ForegroundColor Yellow
    Write-Host "Type what changed in this build, one line per change. Press Enter on an empty line to finish."
    $lines = @()
    while ($true) {
        $line = Read-Host '-'
        if ([string]::IsNullOrWhiteSpace($line)) { break }
        $lines += "- $($line.Trim())"
    }
    if ($lines.Count -eq 0) { $lines = @('- Fixes and improvements.') }
    $notes = $lines -join "`n"
}
$date = Get-Date -Format 'yyyy-MM-dd'
$newSection = "## Unreleased`n`n## $new - $date`n$notes`n`n"
$newLog = $logText.Substring(0, $section.Index) + $newSection + $logText.Substring($section.Index + $section.Length)

# ---- 3. write the new version
$newConf  = [regex]::Replace($confText, '("version"\s*:\s*")\d+\.\d+\.\d+(")', "`${1}$new`${2}")
$cargoRe  = New-Object System.Text.RegularExpressions.Regex '(?m)^version\s*=\s*"\d+\.\d+\.\d+"'
$newCargo = $cargoRe.Replace($cargoText, "version = `"$new`"", 1)

[IO.File]::WriteAllText($conf,  $newConf,  $utf8)
[IO.File]::WriteAllText($cargo, $newCargo, $utf8)
[IO.File]::WriteAllText($log,   $newLog,   $utf8)

Write-Host ""
Write-Host "What's new in $new" -ForegroundColor Cyan
Write-Host $notes
Write-Host ""

# ---- 4. build
Push-Location (Join-Path $root 'src-tauri')
try {
    cargo tauri build
    $ok = ($LASTEXITCODE -eq 0)
} finally {
    Pop-Location
}

if (-not $ok) {
    # Put everything back so the next attempt uses the same number.
    [IO.File]::WriteAllText($conf,  $confText,  $utf8)
    [IO.File]::WriteAllText($cargo, $cargoText, $utf8)
    [IO.File]::WriteAllText($log,   $logText,   $utf8)
    Write-Host "Build failed - version stays $old. Fix the error and run release.ps1 again." -ForegroundColor Red
    exit 1
}

$installer = Get-ChildItem (Join-Path $root 'src-tauri\target\release\bundle\nsis') -Filter "*_${new}_*setup.exe" |
    Select-Object -First 1
Write-Host ""
Write-Host "Done! Respawn $new installer:" -ForegroundColor Green
if ($installer) { Write-Host $installer.FullName } else { Write-Host (Join-Path $root 'src-tauri\target\release\bundle\nsis') }

# ---- 5. publish
if ($Publish) {
    Write-Host ""
    Write-Host "Publishing v$new to GitHub..." -ForegroundColor Cyan
    Push-Location $root
    $failed = $null
    try {
        if ((Invoke-Git add -A) -ne 0) { $failed = 'git add' }
        elseif ((Invoke-Git commit -m "Respawn $new") -ne 0) { $failed = 'git commit' }
        elseif ((Invoke-Git tag "v$new") -ne 0) { $failed = 'git tag' }
        elseif ((Invoke-Git push origin HEAD) -ne 0) { $failed = 'git push' }
        elseif ((Invoke-Git push origin "v$new") -ne 0) { $failed = "git push of tag v$new" }
    } finally { Pop-Location }
    if ($failed) {
        Write-Host "Publishing stopped at: $failed" -ForegroundColor Red
        Write-Host "The installer is built and the version is $new. Fix the problem above, then run:" -ForegroundColor Yellow
        Write-Host "  git add -A; git commit -m `"Respawn $new`"; git tag v$new"
        Write-Host "  git push origin HEAD; git push origin v$new"
        Write-Host "(skip any of those that already worked)"
        exit 1
    }
    $repo = ("$remote".Trim() -replace '\.git$', '') -replace '^git@github\.com:', 'https://github.com/'
    Write-Host ""
    Write-Host "Pushed v$new. GitHub is now building the installer:" -ForegroundColor Green
    Write-Host "  $repo/actions"
    Write-Host "Approve the two signing requests in SignPath when they come in. The release then appears at:"
    Write-Host "  $repo/releases"
}
