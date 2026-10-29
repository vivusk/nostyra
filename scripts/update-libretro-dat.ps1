# Refreshes the bundled identification DATs and the five libretro-database metadat
# attribute DATs (developer, publisher, genre, releaseyear, maxusers) of every
# system from libretro-database at a pinned commit. Manual use only: bump
# $DatCommit, run, review the diff, set METADATA_SOURCE_VERSION in
# crates/game-library/src/metadata_dat_index.rs to the same commit (a test checks it),
# commit. The data is CC-BY-SA-4.0 (libretro-database); DAT data from No-Intro
# (cartridges) and Redump (discs).
# $DatEntries = "<source folder under metadat/>/<name>"; <name> mirrors
# `SystemSpec.dat_file` in crates/system-catalog (a test checks it; each system's
# metadat files use the same name). Vendored flat: third-party/libretro-database/<name>.
# Redump systems: metadat lines "<tab>description ..." (long plot summaries, unused)
# are dropped; an attribute file missing upstream becomes a header-only placeholder
# so the metadata index stays complete. Same output bytes as update-libretro-dat.sh.
$ErrorActionPreference = 'Stop'

$DatCommit = '9a899f5b13ed1c355491e342203be29bc573323b'
$DatEntries = @(
    'no-intro/Nintendo - Super Nintendo Entertainment System.dat'
    'no-intro/Nintendo - Nintendo Entertainment System.dat'
    'no-intro/Nintendo - Game Boy.dat'
    'no-intro/Nintendo - Game Boy Color.dat'
    'no-intro/Nintendo - Game Boy Advance.dat'
    'no-intro/Nintendo - Nintendo 64.dat'
    'no-intro/Nintendo - Nintendo DS.dat'
    'no-intro/Sega - Mega Drive - Genesis.dat'
    'no-intro/Sega - Master System - Mark III.dat'
    'no-intro/Sega - Game Gear.dat'
    'no-intro/Sega - SG-1000.dat'
    'redump/Sony - PlayStation.dat'
    'redump/Sega - Dreamcast.dat'
    'redump/Sony - PlayStation Portable.dat'
    'redump/Nintendo - GameCube.dat'
    'redump/Nintendo - Wii.dat'
)
$MetadatAttrs = @('developer', 'publisher', 'genre', 'releaseyear', 'maxusers')
$RepoRaw = "https://raw.githubusercontent.com/libretro/libretro-database/$DatCommit"
$Utf8NoBom = New-Object System.Text.UTF8Encoding $false

$destDir = Join-Path (Split-Path $PSScriptRoot -Parent) 'third-party/libretro-database'
New-Item -ItemType Directory -Force $destDir | Out-Null

function Write-LfText([string]$Path, [string]$Text) {
    [System.IO.File]::WriteAllText($Path, ($Text -replace "`r`n", "`n"), $Utf8NoBom)
}

foreach ($entry in $DatEntries) {
    $source, $datName = $entry -split '/', 2
    $escapedName = [uri]::EscapeDataString($datName)
    Invoke-WebRequest -Uri "$RepoRaw/metadat/$source/$escapedName" -OutFile (Join-Path $destDir $datName) -UseBasicParsing
    foreach ($attr in $MetadatAttrs) {
        $attrDir = Join-Path $destDir "metadat/$attr"
        New-Item -ItemType Directory -Force $attrDir | Out-Null
        $out = Join-Path $attrDir $datName
        try {
            Invoke-WebRequest -Uri "$RepoRaw/metadat/$attr/$escapedName" -OutFile $out -UseBasicParsing
        }
        catch {
            Write-Warning "no metadat/$attr/$datName upstream; writing a placeholder"
            $system = $datName -replace '\.dat$', ''
            Write-LfText $out "clrmamepro (`n`tname `"$system`"`n`tdescription `"Header only: libretro-database has no $attr metadat for this system`"`n)`n"
            continue
        }
        if ($source -eq 'redump') {
            $text = [System.IO.File]::ReadAllText($out)
            Write-LfText $out (($text -split "(?<=`n)") -notmatch "^`tdescription " -join '')
        }
    }
}
Invoke-WebRequest -Uri "$RepoRaw/LICENSE" -OutFile (Join-Path $destDir 'LICENSE') -UseBasicParsing

$fileLines = ($DatEntries | ForEach-Object { "         $_" }) -join "`n"
$source = @"
Source:  https://github.com/libretro/libretro-database
Commit:  $DatCommit
Files (one set per system, stored as metadat/<source>/<name> as <name>,
       metadat/<attribute>/<name> as metadat/<attribute>/<name>):
$fileLines
         attributes: developer, publisher, genre, releaseyear, maxusers
         redump metadat: "description" lines removed; attributes missing
         upstream are header-only placeholders.
License: CC-BY-SA-4.0 (see LICENSE). Game data by No-Intro (https://no-intro.org)
         and Redump (http://redump.org), compiled by the libretro-database
         contributors.
Use:     bundled data files (unmodified except as noted above). The DATs identify
         dumps by hash (discs also by serial); the metadat files give developer,
         publisher, genre, year and players.
         Refresh with scripts/update-libretro-dat.sh (or .ps1).
"@
# UTF-8 without BOM, LF line ends (same bytes as the .sh version).
Write-LfText (Join-Path $destDir 'SOURCE.txt') ($source + "`n")

Write-Host "DATs + metadat updated: $destDir @ $DatCommit"
