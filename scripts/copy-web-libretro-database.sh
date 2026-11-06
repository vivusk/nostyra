#!/usr/bin/env sh
# Copies the web edition's libretro-database files (DAT + metadat of every
# browser-playable system) into apps/launcher/public/libretro-database/ (git-ignored).
# Needs the wasm module: run scripts/build-web-wasm.sh first. Logic in the .mjs.
set -eu
node "$(dirname "$0")/copy-web-libretro-database.mjs"
