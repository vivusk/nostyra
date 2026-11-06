#!/usr/bin/env sh
# Web edition gate: the browser bundle (apps/launcher/dist-web, `npm run build:web`)
# must not contain Tauri code. Exits 1 when any built file mentions @tauri-apps or
# __TAURI (the desktop backend leaked through the @nostyra/backend alias), or when
# the libretro-database copy, EmulatorJS (scripts/fetch-emulatorjs.*) or the player
# page is missing, or the licences page / notices / browser gate are missing.
set -eu
cd "$(dirname "$0")/../apps/launcher"
[ -d dist-web ] || { echo "dist-web/ missing: run npm run build:web first" >&2; exit 1; }
if grep -rlE "@tauri-apps|__TAURI" dist-web; then
    echo "error: Tauri code in the web bundle (files above)" >&2
    exit 1
fi
for f in LICENSE SOURCE.txt; do
    [ -f "dist-web/libretro-database/$f" ] || {
        echo "error: dist-web/libretro-database/$f missing: run scripts/copy-web-libretro-database first" >&2
        exit 1
    }
done
for f in emulatorjs/4.2.3/LICENSE emulatorjs/4.2.3/SOURCE.txt emulatorjs/4.2.3/loader.js player.html player-boot.js player-input.js; do
    [ -f "dist-web/$f" ] || {
        echo "error: dist-web/$f missing: run scripts/fetch-emulatorjs first" >&2
        exit 1
    }
done
for f in licences.html browser-gate.js static-page.css notices/Nostyra-GPL-3.0.txt notices/THIRD-PARTY-RUST.txt notices/THIRD-PARTY-NPM.txt; do
    [ -f "dist-web/$f" ] || {
        echo "error: dist-web/$f missing: build with scripts/build-web.* (writes public/notices/)" >&2
        exit 1
    }
done
grep -q 'browser-gate.js' dist-web/index.html || {
    echo "error: dist-web/index.html does not load browser-gate.js" >&2
    exit 1
}
if grep -q '__NOSTYRA_VERSION__' dist-web/licences.html; then
    echo "error: dist-web/licences.html still has the version placeholder" >&2
    exit 1
fi
echo "ok: web bundle has no Tauri code and ships the libretro-database copy, EmulatorJS, licences and the browser gate"
