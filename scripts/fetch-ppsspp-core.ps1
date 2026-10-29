# Downloads the nightly ppsspp_libretro core (Windows x86_64) from the libretro
# buildbot into .\cores\, plus the PPSSPP system files (assets/system/PPSSPP.zip)
# into .\cores\PPSSPP\ (the runner copies them to <data>\system\PPSSPP\).
# Windows counterpart of fetch-ppsspp-core.sh.
# Both are unpinned nightlies, so we print their SHA256 for traceability
# instead of verifying against a golden checksum.
$ErrorActionPreference = 'Stop'

# Single place to change if the buildbot layout moves (core-manager mirrors both roots).
$BuildbotBase = 'https://buildbot.libretro.com/nightly'
$BuildbotAssetsRoot = 'https://buildbot.libretro.com'

$repoRoot = Split-Path -Parent $PSScriptRoot
$destDir = Join-Path $repoRoot 'cores'
$lib = 'ppsspp_libretro.dll'
$url = "$BuildbotBase/windows/x86_64/latest/$lib.zip"
$assetsUrl = "$BuildbotAssetsRoot/assets/system/PPSSPP.zip"

$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("nostyra-core-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force $tmpDir | Out-Null
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Write-Host "Fetching $url"
    $zip = Join-Path $tmpDir 'core.zip'
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
    Expand-Archive -Path $zip -DestinationPath (Join-Path $tmpDir 'out') -Force
    $src = Join-Path $tmpDir "out\$lib"
    if (-not (Test-Path $src)) { throw "$lib not found in archive" }

    Write-Host "Fetching $assetsUrl"
    $assetsZip = Join-Path $tmpDir 'assets.zip'
    Invoke-WebRequest -Uri $assetsUrl -OutFile $assetsZip -UseBasicParsing
    $assetsOut = Join-Path $tmpDir 'assets'
    Expand-Archive -Path $assetsZip -DestinationPath $assetsOut -Force
    $assetsSrc = Join-Path $assetsOut 'PPSSPP'
    if (-not (Test-Path (Join-Path $assetsSrc 'compat.ini'))) { throw "PPSSPP/compat.ini not found in archive" }

    New-Item -ItemType Directory -Force $destDir | Out-Null
    $dest = Join-Path $destDir $lib
    Move-Item -Force $src $dest
    $assetsDest = Join-Path $destDir 'PPSSPP'
    if (Test-Path $assetsDest) { Remove-Item -Recurse -Force $assetsDest }
    Move-Item $assetsSrc $assetsDest

    $hash = (Get-FileHash -Algorithm SHA256 $dest).Hash.ToLower()
    Write-Host "$hash  $dest"
    $assetsHash = (Get-FileHash -Algorithm SHA256 $assetsZip).Hash.ToLower()
    Write-Host "$assetsHash  PPSSPP.zip"
    Write-Host "Core ready: $dest"
    Write-Host "System files ready: $assetsDest"
}
finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}
