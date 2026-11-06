# Web edition gate: the browser bundle (apps/launcher/dist-web, `npm run build:web`)
# must not contain Tauri code. Exits 1 when any built file mentions @tauri-apps or
# __TAURI (the desktop backend leaked through the @nostyra/backend alias), or when
# the libretro-database copy, EmulatorJS (scripts/fetch-emulatorjs.*) or the player
# page is missing, or the licences page / notices / browser gate are missing.
$ErrorActionPreference = 'Stop'
$dist = Join-Path (Split-Path -Parent $PSScriptRoot) 'apps\launcher\dist-web'
if (-not (Test-Path $dist)) { throw 'dist-web missing: run npm run build:web first' }
$hits = Get-ChildItem $dist -Recurse -File | Select-String -Pattern '@tauri-apps|__TAURI' -List
if ($hits) {
    $hits | ForEach-Object { Write-Host $_.Path }
    Write-Error 'Tauri code in the web bundle (files above)'
    exit 1
}
foreach ($f in 'LICENSE', 'SOURCE.txt') {
    if (-not (Test-Path (Join-Path $dist "libretro-database\$f"))) {
        Write-Error "dist-web/libretro-database/$f missing: run scripts/copy-web-libretro-database first"
        exit 1
    }
}
foreach ($f in 'emulatorjs\4.2.3\LICENSE', 'emulatorjs\4.2.3\SOURCE.txt', 'emulatorjs\4.2.3\loader.js', 'player.html', 'player-boot.js', 'player-input.js') {
    if (-not (Test-Path (Join-Path $dist $f))) {
        Write-Error "dist-web/$f missing: run scripts/fetch-emulatorjs first"
        exit 1
    }
}
foreach ($f in 'licences.html', 'browser-gate.js', 'static-page.css', 'notices/Nostyra-GPL-3.0.txt', 'notices/THIRD-PARTY-RUST.txt', 'notices/THIRD-PARTY-NPM.txt') {
    if (-not (Test-Path (Join-Path $dist $f))) {
        Write-Error "dist-web/$f missing: build with scripts/build-web.* (writes public/notices/)"
        exit 1
    }
}
if (-not (Select-String -Path (Join-Path $dist 'index.html') -Pattern 'browser-gate.js' -SimpleMatch -Quiet)) {
    Write-Error 'dist-web/index.html does not load browser-gate.js'
    exit 1
}
if (Select-String -Path (Join-Path $dist 'licences.html') -Pattern '__NOSTYRA_VERSION__' -SimpleMatch -Quiet) {
    Write-Error 'dist-web/licences.html still has the version placeholder'
    exit 1
}
Write-Host 'ok: web bundle has no Tauri code and ships the libretro-database copy, EmulatorJS, licences and the browser gate'
