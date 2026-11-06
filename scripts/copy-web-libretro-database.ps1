# Copies the web edition's libretro-database files (DAT + metadat of every
# browser-playable system) into apps/launcher/public/libretro-database/ (git-ignored).
# Needs the wasm module: run scripts/build-web-wasm.ps1 first. Logic in the .mjs.
$ErrorActionPreference = 'Stop'
node (Join-Path $PSScriptRoot 'copy-web-libretro-database.mjs')
if ($LASTEXITCODE -ne 0) { throw 'copy-web-libretro-database failed' }
