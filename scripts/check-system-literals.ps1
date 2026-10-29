# M6 exit-criterion gate: system and core facts live only in crates/system-catalog.
# Prints every non-test line outside it that hard-codes a system/core id ("snes",
# "nes", "gb", "gbc", "gba", "psx", "genesis", "sms", "gg", "sg1000", "snes9x",
# "mesen", "mgba", "pcsx-rearmed", "pcsx_rearmed", "genesis_plus_gx", "n64",
# "mupen64plus-next", "mupen64plus_next", "dreamcast", "flycast", "flycast_libretro",
# "psp", "ppsspp", "ppsspp_libretro", "nds", "melonds_ds", "melondsds_libretro",
# "gc", "wii", "dolphin-emu", "dolphin_libretro", and the upper-case labels), a
# Flycast, PPSSPP, melonDS DS or Dolphin core option ("reicast_*", "ppsspp_*",
# "melonds_*", "dolphin_*") or a ROM extension inside a string (".sfc", ".nes",
# ".gb", ".gbc", ".gba", ".z64", ".v64", ".n64", ".md", ".gen", ".smd", ".sms",
# ".gg", ".sg", ".cue", ".bin", ".iso", ".chd", ".pbp", ".m3u", ".gdi", ".cdi",
# ".cso", ".nds", ".dsi", ".ids", ".gcm", ".ciso", ".gcz", ".rvz", ".wia", ".wbfs",
# ".dol", ".elf"). crates/disc-image keeps its extension constants
# dot-less ("cue").
# Exit 1 when something is found.
#
# Scope: crates/**/src/**/*.rs, apps/**/src/**/*.rs, apps/launcher/src/**/*.ts(x).
# Skipped: crates/system-catalog, tests/ dirs, *_tests.rs / *.test.ts(x) files,
# comment lines, and in Rust files every line from the first `#[cfg(test)]` on.
# Same rules as check-system-literals.sh.
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

# Intentional hits: path => reason.
$Allow = @{
    'apps/runner/src/achievements_real_core_loader.rs' = 'test-only module (#[cfg(test)] mod in main.rs)'
    'apps/runner/src/runner_real_core_suite.rs'        = 'test-only module (#[cfg(test)] mod in main.rs)'
    'crates/disc-image/src/test_gd_images.rs'          = 'test-only fixtures (cfg(test) / feature test-images): Redump and TOSEC track file names'
    'crates/game-library/src/clrmamepro_dat_parser.rs' = 'DAT format: libretro NES DAT names headered ROMs *.nes'
    'crates/ipc-protocol/src/shared_data_paths.rs'     = 'RTC_FILE_NAME = "rtc.bin": RTC save file name, not a ROM extension'
}

$pattern = '"(nes|snes|snes9x|mesen|SNES|NES|gb|gbc|gba|mgba|GB|GBC|GBA|psx|pcsx-rearmed|pcsx_rearmed|PS1|n64|N64|mupen64plus-next|mupen64plus_next|genesis|sms|gg|sg1000|genesis_plus_gx|MD|SMS|GG|SG|dreamcast|flycast|flycast_libretro|DC|psp|ppsspp|ppsspp_libretro|PSP|nds|NDS|melonds_ds|melondsds_libretro|gc|GC|wii|Wii|dolphin-emu|dolphin_libretro)"|"reicast_[^"]*"|"ppsspp_[^"]*"|"melonds_[^"]*"|"dolphin_[^"]*"|"[^"]*\.(sfc|nes|gb|gbc|gba|z64|v64|n64|md|gen|smd|sms|gg|sg|cue|bin|iso|chd|pbp|m3u|gdi|cdi|cso|nds|dsi|ids|gcm|ciso|gcz|rvz|wia|wbfs|dol|elf)[^"]*"'
$files = @(Get-ChildItem -Recurse -File -Path crates, apps -Include *.rs |
        Where-Object { $_.FullName -match '[\\/]src[\\/]' -and $_.FullName -notmatch '[\\/]target[\\/]' })
$files += @(Get-ChildItem -Recurse -File -Path apps/launcher/src -Include *.ts, *.tsx)

$found = $false
foreach ($file in $files) {
    $rel = (Resolve-Path -Relative $file.FullName).TrimStart('.', '\', '/') -replace '\\', '/'
    if ($rel -like 'crates/system-catalog/*' -or $rel -like '*/tests/*' -or $rel -like '*_tests.rs' -or
        $rel -like '*.test.ts' -or $rel -like '*.test.tsx' -or $rel -like '*/node_modules/*' -or
        $Allow.ContainsKey($rel)) {
        continue
    }
    $lineNo = 0
    foreach ($line in [System.IO.File]::ReadLines($file.FullName)) {
        $lineNo++
        if ($line -match '^\s*#\[cfg\(test\)\]') { break }
        if ($line -match '^\s*(//|\*|/\*)') { continue }
        if ($line -cmatch $pattern) {
            Write-Output "${rel}:${lineNo}: $($line.Trim())"
            $found = $true
        }
    }
}

if ($found) {
    Write-Error 'system/core literals outside crates/system-catalog (see above)'
    exit 1
}
Write-Output 'ok: no system/core literals outside crates/system-catalog'
