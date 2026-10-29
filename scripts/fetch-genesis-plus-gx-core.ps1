# Downloads the nightly genesis_plus_gx_libretro core (Windows x86_64) from the libretro
# buildbot into .\cores\. Windows counterpart of fetch-genesis-plus-gx-core.sh.
# The core is an unpinned nightly, so we print its SHA256 for traceability
# instead of verifying against a golden checksum.
$ErrorActionPreference = 'Stop'

# Single place to change if the buildbot layout moves.
$BuildbotBase = 'https://buildbot.libretro.com/nightly'

$repoRoot = Split-Path -Parent $PSScriptRoot
$destDir = Join-Path $repoRoot 'cores'
$lib = 'genesis_plus_gx_libretro.dll'
$url = "$BuildbotBase/windows/x86_64/latest/$lib.zip"

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

    New-Item -ItemType Directory -Force $destDir | Out-Null
    $dest = Join-Path $destDir $lib
    Move-Item -Force $src $dest
    $hash = (Get-FileHash -Algorithm SHA256 $dest).Hash.ToLower()
    Write-Host "$hash  $dest"
    Write-Host "Core ready: $dest"
}
finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}
