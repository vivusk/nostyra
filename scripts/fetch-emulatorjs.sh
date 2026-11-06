#!/usr/bin/env sh
# Vendors EmulatorJS 4.2.3 (runtime + the web cores, sha256-pinned) into
# apps/launcher/public/emulatorjs/4.2.3/ (git-ignored). Needs the wasm module (the
# core list comes from the registry): run scripts/build-web-wasm.sh first.
# Logic in the .mjs.
set -eu
node "$(dirname "$0")/fetch-emulatorjs.mjs"
