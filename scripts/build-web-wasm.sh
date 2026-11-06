#!/usr/bin/env sh
# Builds the web edition's WebAssembly module (crates/web-bindings) into
# apps/launcher/src/backend/web/wasm-pkg/ (generated, git-ignored) with wasm-pack
# (0.15.0: cargo install wasm-pack --version 0.15.0 --locked). Release build, wasm-opt
# by wasm-pack. Prints the raw and gzip sizes of the .wasm.
set -eu
cd "$(dirname "$0")/.."
out="$(pwd)/apps/launcher/src/backend/web/wasm-pkg"
command -v wasm-pack >/dev/null || {
    echo "wasm-pack not found: cargo install wasm-pack --version 0.15.0 --locked" >&2
    exit 1
}
wasm-pack build crates/web-bindings --target web --release --no-pack \
    --out-dir "$out" --out-name nostyra_web
wasm="$out/nostyra_web_bg.wasm"
echo "nostyra_web_bg.wasm: $(wc -c < "$wasm") bytes, $(gzip -9c "$wasm" | wc -c) gzip"
