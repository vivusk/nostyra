# Builds the Windows portable copy: .\dist\Nostyra-<version>-windows-x64-portable.zip
# (download, unzip, run Nostyra.exe; no installer). Layout beside the exes is what an
# unbundled Tauri app reads as its resource dir: libretro-database\ and shaders\, plus
# the Visual C++ runtime DLLs the runner needs.
# portable.txt makes both exes keep their data in .\data\ (ipc_protocol::portable_dir).
# Needs the WebView2 runtime on the target PC (Windows 11; Windows 10 since 2021).
# Licence notices need cargo-about 0.9.2 (`cargo install cargo-about --version 0.9.2
# --locked --features cli`), Node.js and apps\launcher\node_modules (npm ci).
param(
    # Reuse target\release\*.exe from an earlier build.
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$conf = Get-Content (Join-Path $repoRoot 'apps\launcher\src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$name = "Nostyra-$($conf.version)-windows-x64-portable"
$distDir = Join-Path $repoRoot 'dist'
$stage = Join-Path $distDir $name
$release = Join-Path $repoRoot 'target\release'
$database = Join-Path $repoRoot 'third-party\libretro-database'
$releaseNotes = Join-Path $repoRoot "docs\release-notes-v$($conf.version).md"
if (-not (Test-Path $releaseNotes)) { throw "missing $releaseNotes (write the release notes first)" }
if (-not (Get-Command cargo-about -ErrorAction SilentlyContinue)) {
    throw 'cargo-about not found: cargo install cargo-about --version 0.9.2 --locked --features cli'
}

if (-not $SkipBuild) {
    Push-Location $repoRoot
    # cargo / npm print progress on stderr: Windows PowerShell turns redirected stderr
    # into errors, so judge the tools by their exit codes only.
    $ErrorActionPreference = 'Continue'
    try {
        cargo build --release -p nostyra-runner
        if ($LASTEXITCODE -ne 0) { throw 'runner build failed' }
        Push-Location 'apps\launcher'
        # --no-bundle: release exe + frontend only, no MSI/NSIS.
        npx tauri build --no-bundle
        if ($LASTEXITCODE -ne 0) { throw 'launcher build failed' }
        Pop-Location
    } finally {
        Pop-Location
        $ErrorActionPreference = 'Stop'
    }
}

# The stage folder is also a runnable portable copy: if someone ran it, it holds a
# `data\` folder (library, saves, logs). Never delete that; ask to move it first.
if (Test-Path (Join-Path $stage 'data')) {
    throw "$stage\data exists (this portable copy was run). Close Nostyra, move that data folder somewhere safe, then build again."
}
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force (Join-Path $stage 'libretro-database') | Out-Null

Copy-Item (Join-Path $release 'nostyra-launcher.exe') (Join-Path $stage 'Nostyra.exe')
Copy-Item (Join-Path $release 'nostyra-runner.exe') $stage
# The runner (C++ deps) links the Visual C++ runtime dynamically; a fresh Windows may
# lack it, so ship it app-local (redistributable files, loaded from the exe folder).
# The launcher links it statically (tauri-build) and needs only the built-in UCRT.
foreach ($dll in 'vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll') {
    $source = Join-Path $env:SystemRoot "System32\$dll"
    if (-not (Test-Path $source)) { throw "$dll not found; install the Visual C++ Redistributable (x64)" }
    Copy-Item $source $stage
}
# Same resources as tauri.conf.json bundle.resources: top-level DAT files + metadat\.
Get-ChildItem $database -File | Copy-Item -Destination (Join-Path $stage 'libretro-database')
Copy-Item -Recurse (Join-Path $database 'metadat') (Join-Path $stage 'libretro-database\metadat')
Copy-Item -Recurse (Join-Path $repoRoot 'third-party\slang-shaders') (Join-Path $stage 'shaders')
# Licence texts that must travel with the binaries: Nostyra itself, rcheevos (MIT, compiled
# into the runner and launcher) and the Noto Sans fonts (OFL-1.1, embedded in the runner).
$licenses = Join-Path $stage 'licenses'
New-Item -ItemType Directory -Force $licenses | Out-Null
Copy-Item (Join-Path $repoRoot 'LICENSE') (Join-Path $licenses 'Nostyra-GPL-3.0.txt')
Copy-Item (Join-Path $repoRoot 'third-party\rcheevos\LICENSE') (Join-Path $licenses 'rcheevos-MIT.txt')
Copy-Item (Join-Path $repoRoot 'assets\fonts\OFL.txt') (Join-Path $licenses 'NotoSans-OFL-1.1.txt')
# Bundled data: libretro-database (CC-BY-SA-4.0) and slang-shaders (per-file headers).
Copy-Item (Join-Path $database 'LICENSE') (Join-Path $licenses 'libretro-database-LICENSE.txt')
Copy-Item (Join-Path $database 'SOURCE.txt') (Join-Path $licenses 'libretro-database-SOURCE.txt')
Copy-Item (Join-Path $repoRoot 'third-party\slang-shaders\SOURCE.txt') (Join-Path $licenses 'slang-shaders-SOURCE.txt')
# Generated notices: Rust crates (about.toml / about.hbs), native C/C++ libraries + other
# shipped files (SDL3, glslang, SPIRV-Cross, SQLite, VC++ runtime), npm packages of the UI.
Push-Location $repoRoot
$ErrorActionPreference = 'Continue'
try {
    cargo about generate --locked --fail about.hbs -o (Join-Path $licenses 'THIRD-PARTY-RUST.txt')
    if ($LASTEXITCODE -ne 0) { throw 'cargo about generate failed' }
    node (Join-Path $PSScriptRoot 'generate-npm-notices.mjs') (Join-Path $licenses 'THIRD-PARTY-NPM.txt')
    if ($LASTEXITCODE -ne 0) { throw 'npm notices failed' }
    & (Join-Path $PSScriptRoot 'generate-native-notices.ps1') -OutFile (Join-Path $licenses 'THIRD-PARTY-NATIVE.txt')
} finally {
    Pop-Location
    $ErrorActionPreference = 'Stop'
}
Copy-Item $releaseNotes (Join-Path $stage 'RELEASE-NOTES.md')
Set-Content -Encoding utf8 (Join-Path $stage 'portable.txt') @'
Nostyra portable copy.

This file keeps Nostyra in this folder: your library, saves, save states, cores and
settings live in the "data" folder next to Nostyra.exe. Copy or move the whole folder
to take everything with you. Delete this file to use %APPDATA%\Nostyra\data instead.

Needs the Microsoft Edge WebView2 Runtime (built into Windows 11 and updated Windows 10):
https://go.microsoft.com/fwlink/p/?LinkId=2124703
'@

$zip = Join-Path $distDir "$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip
$size = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host "Portable zip: $zip ($size MB)"
