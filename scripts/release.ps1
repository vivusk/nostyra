# Creates a release on `main` from the current `dev` tree (one commit per release).
#
# Branch model: daily work happens on `dev` (local only). A release is a single
# commit on `main` whose tree is exactly dev's tree at that moment, with the
# previous release as its parent (the first release has no parent). Built with
# `git commit-tree`, so it never conflicts and files deleted on dev disappear
# from main too. Only `main` and the release tags are pushed to GitHub.
#
# Usage:  powershell -File scripts/release.ps1 0.2.0
# Before: bump the version on dev in Cargo.toml ([workspace.package]),
#         apps/launcher/src-tauri/tauri.conf.json and apps/launcher/package.json,
#         add release notes docs/release-notes-v<version>.md, commit on dev.
# The script prints the push and build commands; it never pushes.

param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

function Fail([string]$message) {
    Write-Host "release: $message" -ForegroundColor Red
    exit 1
}

function Invoke-Git {
    $output = & git.exe @args
    if ($LASTEXITCODE -ne 0) { Fail "git $($args -join ' ') failed" }
    return $output
}

# 1. On a clean dev branch.
$branch = (Invoke-Git branch --show-current).Trim()
if ($branch -ne 'dev') { Fail "run on branch 'dev' (current: '$branch')" }
if (Invoke-Git status --porcelain) { Fail 'working tree not clean; commit or stash first' }

# 2. Version bumped everywhere it is read (RetroAchievements needs an increasing
#    numeric version in the User-Agent, which comes from CARGO_PKG_VERSION).
$cargoVersion = (Select-String -Path 'Cargo.toml' -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value
$tauriVersion = (Get-Content 'apps/launcher/src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json).version
$npmVersion = (Get-Content 'apps/launcher/package.json' -Raw | ConvertFrom-Json).version
foreach ($pair in @(
        @('Cargo.toml', $cargoVersion),
        @('apps/launcher/src-tauri/tauri.conf.json', $tauriVersion),
        @('apps/launcher/package.json', $npmVersion))) {
    if ($pair[1] -ne $Version) { Fail "$($pair[0]) has version $($pair[1]), expected $Version" }
}
if (-not (Test-Path "docs/release-notes-v$Version.md")) { Fail "missing docs/release-notes-v$Version.md" }

# 3. Tag must be new.
$tag = "v$Version"
& git.exe rev-parse -q --verify "refs/tags/$tag" *> $null
if ($LASTEXITCODE -eq 0) { Fail "tag $tag already exists" }

# 4. One commit on main with dev's exact tree.
$tree = (Invoke-Git rev-parse 'dev^{tree}').Trim()
& git.exe rev-parse -q --verify 'refs/heads/main' *> $null
$hasMain = ($LASTEXITCODE -eq 0)
$message = "Nostyra $tag"
if ($hasMain) {
    $mainTree = (Invoke-Git rev-parse 'main^{tree}').Trim()
    if ($mainTree -eq $tree) { Fail 'main already has this exact tree; nothing to release' }
    $commit = ($message | & git.exe commit-tree $tree -p main).Trim()
} else {
    $commit = ($message | & git.exe commit-tree $tree).Trim()
}
if ($LASTEXITCODE -ne 0 -or -not $commit) { Fail 'git commit-tree failed' }
if ($hasMain) {
    Invoke-Git update-ref -m "release $tag" refs/heads/main $commit (Invoke-Git rev-parse main).Trim() | Out-Null
} else {
    Invoke-Git update-ref -m "release $tag" refs/heads/main $commit | Out-Null
}
Invoke-Git tag -a $tag $commit -m $message | Out-Null

Write-Host "release: main -> $($commit.Substring(0, 7)) ($message), tag $tag" -ForegroundColor Green
Write-Host ''
Write-Host 'Next steps:'
Write-Host '  1. Build the zip (dev has the same tree as the release commit):'
Write-Host '       powershell -File scripts/build-portable.ps1'
Write-Host '  2. Push only main and the tag:'
Write-Host "       git push origin main $tag"
Write-Host "  3. GitHub -> Releases -> Draft a new release -> tag $tag,"
Write-Host "     paste docs/release-notes-v$Version.md, attach dist/Nostyra-$Version-windows-x64-portable.zip"
