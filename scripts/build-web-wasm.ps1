# Builds the web edition's WebAssembly module (crates/web-bindings) into
# apps/launcher/src/backend/web/wasm-pkg/ (generated, git-ignored) with wasm-pack
# (0.15.0: cargo install wasm-pack --version 0.15.0 --locked). Release build, wasm-opt
# by wasm-pack. Prints the raw and gzip sizes of the .wasm.
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $repoRoot 'apps\launcher\src\backend\web\wasm-pkg'
if (-not (Get-Command wasm-pack -ErrorAction SilentlyContinue)) {
    throw 'wasm-pack not found: cargo install wasm-pack --version 0.15.0 --locked'
}
wasm-pack build (Join-Path $repoRoot 'crates\web-bindings') --target web --release --no-pack `
    --out-dir $outDir --out-name nostyra_web
if ($LASTEXITCODE -ne 0) { throw 'wasm-pack build failed' }
$wasm = Join-Path $outDir 'nostyra_web_bg.wasm'
$bytes = [IO.File]::ReadAllBytes($wasm)
$buffer = New-Object IO.MemoryStream
$gzip = New-Object IO.Compression.GZipStream($buffer, [IO.Compression.CompressionLevel]::Optimal)
$gzip.Write($bytes, 0, $bytes.Length)
$gzip.Close()
Write-Host ("nostyra_web_bg.wasm: {0:N0} bytes, {1:N0} gzip" -f $bytes.Length, $buffer.ToArray().Length)
