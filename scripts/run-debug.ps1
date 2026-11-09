# Rebuilds the debug launcher + runner and starts the debug launcher.
# From the repo root:  .\scripts\run-debug.ps1
# The launcher goes through `tauri build --debug` (not plain cargo build): that enables
# tauri's `custom-protocol` feature so the exe serves the embedded frontend; a plain
# `cargo build` leaves the launcher in dev mode, pointing at devUrl (localhost:1420,
# "connection refused"). It also runs beforeBuildCommand (`npm run build`).
# Hot reload is not needed for this flow; use `npm run tauri dev` in apps\launcher for that.
param(
    # Build only, do not start the launcher.
    [switch]$NoRun
)
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$launcherDir = Join-Path $repoRoot 'apps\launcher'

if (-not (Test-Path (Join-Path $launcherDir 'node_modules'))) {
    throw "apps\launcher\node_modules missing: run `npm ci` in apps\launcher first"
}

# A debug launcher/runner still running from target\debug locks its exe ("Access is
# denied" on relink). Only processes from this repo's target\debug are stopped.
$debugDir = Join-Path $repoRoot 'target\debug'
Get-Process nostyra-launcher, nostyra-runner -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($debugDir, [StringComparison]::OrdinalIgnoreCase) } |
    ForEach-Object {
        Write-Host "stopping $($_.Path) (pid $($_.Id))"
        Stop-Process -Id $_.Id -Force
    }

Push-Location $repoRoot
# cargo / npm print progress on stderr: Windows PowerShell turns redirected stderr
# into errors, so judge the tools by their exit codes only.
$ErrorActionPreference = 'Continue'
try {
    cargo build -p nostyra-runner
    if ($LASTEXITCODE -ne 0) { throw 'runner build failed' }
    Push-Location $launcherDir
    # --no-bundle: exe only, no MSI/NSIS.
    npx tauri build --debug --no-bundle
    $code = $LASTEXITCODE
    Pop-Location
    if ($code -ne 0) { throw 'launcher build failed' }
    $targetDir = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
} finally {
    Pop-Location
    $ErrorActionPreference = 'Stop'
}

$exe = Join-Path $targetDir 'debug\nostyra-launcher.exe'
if (-not (Test-Path $exe)) { throw "missing $exe" }
if ($NoRun) { Write-Host "built $exe"; exit 0 }

Write-Host "starting $exe"
# Detached so the terminal is free; logs go to <data>\logs\ as usual.
Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe)
