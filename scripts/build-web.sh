#!/usr/bin/env sh
# Builds the deployable web edition (nostyra.com/online): wasm → EmulatorJS (verified) →
# libretro-database copy → licence notices → npm run build:web → bundle check →
# dist/nostyra-web-<version>/ (online/, vercel.headers.json, DEPLOY.md) + zip; prints the
# size of each part. Logic in build-web.mjs (this wrapper uses the .sh helpers).
set -eu
node "$(dirname "$0")/build-web.mjs" --shell=sh
