# Builds the deployable web edition (nostyra.com/online): wasm → EmulatorJS (verified) →
# libretro-database copy → licence notices → npm run build:web → bundle check →
# dist\nostyra-web-<version>\ (online\, vercel.headers.json, DEPLOY.md) + zip; prints the
# size of each part. Logic in build-web.mjs (this wrapper uses the .ps1 helpers).
$ErrorActionPreference = 'Stop'
node (Join-Path $PSScriptRoot 'build-web.mjs') --shell=ps1
if ($LASTEXITCODE -ne 0) { throw 'build-web failed' }
