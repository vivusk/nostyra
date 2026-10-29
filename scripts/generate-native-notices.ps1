# Writes the licence texts of the native C/C++ libraries compiled into the Nostyra
# executables (vendored inside -sys crates, so cargo-about's crate-level scan misses
# them) and notes for the other files the portable zip ships.
# Usage: .\scripts\generate-native-notices.ps1 -OutFile <path>\THIRD-PARTY-NATIVE.txt
# Called by build-portable.ps1. Reads crate sources from the cargo registry (Cargo.lock
# versions; run a cargo build or `cargo fetch` first).
param(
    [Parameter(Mandatory = $true)][string]$OutFile
)
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $json = cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
} finally {
    Pop-Location
}
$packages = ($json | Out-String | ConvertFrom-Json).packages

function Get-CrateDir([string]$crate) {
    $found = @($packages | Where-Object { $_.name -eq $crate })
    if ($found.Count -ne 1) { throw "expected one '$crate' package in Cargo.lock, found $($found.Count)" }
    return @{ Dir = Split-Path -Parent $found[0].manifest_path; Version = $found[0].version }
}

# Library, linked into, crate holding the source, licence files relative to that crate.
$libraries = @(
    @{ Name = 'SDL3'; Licence = 'Zlib'; Use = 'Nostyra.exe and nostyra-runner.exe (static; window, audio, gamepads)'
       Crate = 'sdl3-src'; Files = @('SDL/LICENSE.txt'); Url = 'https://libsdl.org' },
    @{ Name = 'glslang'; Licence = 'BSD-3-Clause, BSD-2-Clause, MIT, Apache-2.0 and GPL-3.0-or-later WITH Bison-exception-2.2 (see text)'
       Use = 'nostyra-runner (static; shader compiler used by librashader)'
       Crate = 'glslang-sys'; Files = @('native/glslang/LICENSE.txt'); Url = 'https://github.com/KhronosGroup/glslang' },
    @{ Name = 'SPIRV-Cross'; Licence = 'Apache-2.0 (some headers MIT / Khronos free use; docs CC-BY-4.0)'
       Use = 'nostyra-runner (static; shader cross-compiler used by librashader)'
       Crate = 'spirv-cross-sys'; Files = @('native/SPIRV-Cross/LICENSE', 'native/SPIRV-Cross/LICENSES/MIT.txt',
           'native/SPIRV-Cross/LICENSES/LicenseRef-KhronosFreeUse.txt'); Url = 'https://github.com/KhronosGroup/SPIRV-Cross' }
)

$out = New-Object System.Text.StringBuilder
[void]$out.AppendLine(@'
Nostyra - native libraries and other bundled files
==================================================

Native C/C++ libraries compiled into the executables
----------------------------------------------------
SQLite (public domain) - compiled into both executables through the rusqlite / libsqlite3-sys
  crates (bundled amalgamation). The SQLite authors dedicated the code to the public
  domain; no licence text is required. https://sqlite.org/copyright.html
rcheevos (MIT) - compiled into both executables; licence text in rcheevos-MIT.txt.
'@)
foreach ($lib in $libraries) {
    $crate = Get-CrateDir $lib.Crate
    [void]$out.AppendLine('=' * 80)
    [void]$out.AppendLine("$($lib.Name) - $($lib.Licence)")
    [void]$out.AppendLine("Used in: $($lib.Use)")
    [void]$out.AppendLine("Source: $($lib.Url) (as vendored in the $($lib.Crate) $($crate.Version) crate)")
    foreach ($file in $lib.Files) {
        $path = Join-Path $crate.Dir $file
        if (-not (Test-Path $path)) { throw "licence file missing: $path" }
        [void]$out.AppendLine('-' * 80)
        [void]$out.AppendLine((Get-Content -Raw -Encoding utf8 $path).TrimEnd())
    }
    [void]$out.AppendLine()
}
[void]$out.AppendLine(@'
================================================================================
Other files in this folder
--------------------------
vcruntime140.dll, vcruntime140_1.dll, msvcp140.dll - Microsoft Visual C++ runtime,
  Copyright (c) Microsoft Corporation. Redistributable files shipped unmodified under
  the Visual Studio redistribution terms ("Distributable Code"); they are not covered
  by Nostyra's licence. https://learn.microsoft.com/cpp/windows/redistributing-visual-cpp-files
libretro-database\ - No-Intro / Redump DATs and metadat files compiled by
  libretro-database, CC-BY-SA-4.0: see libretro-database-LICENSE.txt and
  libretro-database-SOURCE.txt (source commit; the PlayStation metadat file has its
  long description lines removed). https://github.com/libretro/libretro-database
shaders\ - CRT / LCD presets from libretro/slang-shaders, unmodified, each file keeps
  its licence header (GPL / GPL-2.0-or-later / public domain): see
  slang-shaders-SOURCE.txt. https://github.com/libretro/slang-shaders
Noto Sans fonts (embedded in nostyra-runner.exe) - SIL OFL-1.1: NotoSans-OFL-1.1.txt.
Third-party Rust crates: THIRD-PARTY-RUST.txt. Launcher web UI (npm packages):
  THIRD-PARTY-NPM.txt.
Emulator cores, their system files, BIOS files and games are not included; cores are
downloaded from the libretro buildbot on first use, each under its own licence.
'@)

$dir = Split-Path -Parent $OutFile
if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }
[System.IO.File]::WriteAllText($OutFile, $out.ToString(), (New-Object System.Text.UTF8Encoding $false))
Write-Host "Native notices: $OutFile"
