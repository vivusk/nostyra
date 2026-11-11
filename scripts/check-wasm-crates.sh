#!/usr/bin/env sh
# Checks that the crates the web edition compiles to WebAssembly build for
# wasm32-unknown-unknown without their native features (ipc-protocol `native`,
# config-layers `fs`). Target from rust-toolchain.toml; separate target dir so the
# desktop build cache is untouched. Exits non-zero on the first failure.
set -eu
cd "$(dirname "$0")/.."
common="--target wasm32-unknown-unknown --target-dir target/wasm-check"
for args in \
    "-p content-file-source -p system-catalog -p rom-patch -p disc-image -p rom-identity -p web-bindings -p netplay-wire" \
    "-p ipc-protocol --no-default-features" \
    "-p config-layers --no-default-features"; do
    echo "cargo clippy $args"
    # shellcheck disable=SC2086 # word splitting of the argument lists is intended
    cargo clippy $args $common -- -D warnings
done
echo "wasm32 crates OK"
