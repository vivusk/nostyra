# Checks that the crates the web edition compiles to WebAssembly build for
# wasm32-unknown-unknown without their native features (ipc-protocol `native`,
# config-layers `fs`). Target from rust-toolchain.toml; separate target dir so the
# desktop build cache is untouched. Exit code != 0 on the first failure.
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $common = @('--target', 'wasm32-unknown-unknown', '--target-dir', 'target/wasm-check')
    $checks = @(
        @('-p', 'content-file-source', '-p', 'system-catalog', '-p', 'rom-patch', '-p', 'disc-image', '-p', 'rom-identity', '-p', 'web-bindings'),
        @('-p', 'ipc-protocol', '--no-default-features'),
        @('-p', 'config-layers', '--no-default-features')
    )
    foreach ($crateArgs in $checks) {
        Write-Host "cargo clippy $($crateArgs -join ' ')"
        cargo clippy @crateArgs @common -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw "wasm32 check failed: $($crateArgs -join ' ')" }
    }
    Write-Host 'wasm32 crates OK'
}
finally { Pop-Location }
