# Vendors EmulatorJS 4.2.3 (runtime + the web cores, sha256-pinned) into
# apps/launcher/public/emulatorjs/4.2.3/ (git-ignored). Needs the wasm module (the
# core list comes from the registry): run scripts/build-web-wasm.ps1 first.
# Logic in the .mjs.
$ErrorActionPreference = 'Stop'
node (Join-Path $PSScriptRoot 'fetch-emulatorjs.mjs')
if ($LASTEXITCODE -ne 0) { throw 'fetch-emulatorjs failed' }
