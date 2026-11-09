#!/bin/bash
# Builds a distributable macOS release of Nostyra that runs on any Mac:
#   universal (Apple Silicon + Intel) Nostyra.app with nostyra-runner inside,
#   signed with Developer ID + hardened runtime, notarized by Apple, stapled,
#   packed as dist/Nostyra-<version>-macos-universal.dmg (+ .sha256).
#
# One-time setup:
#   - "Developer ID Application" certificate of $NOSTYRA_TEAM_ID in the login keychain
#     (Xcode → Settings → Accounts → Manage Certificates → + → Developer ID Application).
#   - notarytool credentials: xcrun notarytool store-credentials "<profile>" \
#       --apple-id <email> --team-id <team> --password <app-specific password>
#   - Optional: NOSTYRA_GOOGLE_CLIENT_ID / NOSTYRA_GOOGLE_CLIENT_SECRET in the environment
#     to embed Google Drive cloud saves (never commit them).
#
# Env overrides: NOSTYRA_TEAM_ID, NOSTYRA_NOTARY_PROFILE, SKIP_NOTARIZE=1 (sign only).
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT=$(pwd)

TEAM="${NOSTYRA_TEAM_ID:-LVJ9374KGW}"
PROFILE="${NOSTYRA_NOTARY_PROFILE:-macxa}"
RUNNER_ENTITLEMENTS="apps/launcher/src-tauri/entitlements/nostyra-runner-macos.entitlements"
VERSION=$(node -p "require('./apps/launcher/src-tauri/tauri.conf.json').version")
DIST="$ROOT/dist"
APP="$ROOT/target/universal-apple-darwin/release/bundle/macos/Nostyra.app"
DMG="$DIST/Nostyra-$VERSION-macos-universal.dmg"

echo "▸ Checking prerequisites (version $VERSION)"
# Pick the Developer ID of exactly this team: a machine may hold several.
DEV_ID=$(security find-identity -v -p codesigning | grep "Developer ID Application" \
         | grep -F "($TEAM)" | awk '{print $2}' | head -1 || true)
[ -n "$DEV_ID" ] || { echo "  ✗ no 'Developer ID Application' certificate of team $TEAM"; exit 1; }
echo "  certificate: $(security find-identity -v -p codesigning | grep "$DEV_ID" | head -1 | cut -d'"' -f2)"
if [ "${SKIP_NOTARIZE:-0}" != "1" ]; then
    xcrun notarytool history --keychain-profile "$PROFILE" >/dev/null 2>&1 \
        || { echo "  ✗ notarytool profile '$PROFILE' missing (see header)"; exit 1; }
    echo "  notary profile: $PROFILE"
fi
if [ -z "${NOSTYRA_GOOGLE_CLIENT_ID:-}" ] || [ -z "${NOSTYRA_GOOGLE_CLIENT_SECRET:-}" ]; then
    echo "  ⚠ NOSTYRA_GOOGLE_CLIENT_ID/SECRET not set: this build has no Google Drive cloud saves"
fi
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null

echo "▸ Building launcher UI"
(cd apps/launcher && npm ci --silent && npm run build >/dev/null)

echo "▸ Building universal nostyra-runner"
for t in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build -p nostyra-runner --release --target "$t"
done
RUNNER="$ROOT/target/universal-apple-darwin/release/nostyra-runner"
mkdir -p "$(dirname "$RUNNER")"
lipo -create -output "$RUNNER" \
    target/aarch64-apple-darwin/release/nostyra-runner \
    target/x86_64-apple-darwin/release/nostyra-runner

echo "▸ Building universal Nostyra.app"
(cd apps/launcher && npx tauri build --target universal-apple-darwin --bundles app)
[ -d "$APP" ] || { echo "  ✗ $APP not found after build"; exit 1; }
# The launcher spawns nostyra-runner from beside its own executable.
cp "$RUNNER" "$APP/Contents/MacOS/nostyra-runner"

echo "▸ Signing (Developer ID + hardened runtime, inside-out)"
codesign --force --timestamp --options runtime --entitlements "$RUNNER_ENTITLEMENTS" \
    --sign "$DEV_ID" "$APP/Contents/MacOS/nostyra-runner"
codesign --force --timestamp --options runtime --sign "$DEV_ID" "$APP"
codesign --verify --strict --deep --verbose=2 "$APP" 2>&1 | sed 's/^/  /'
lipo -info "$APP/Contents/MacOS/nostyra-launcher" "$APP/Contents/MacOS/nostyra-runner" | sed 's/^/  /'

notarize() { # $1 = file to submit
    local log; log=$(mktemp)
    if ! xcrun notarytool submit "$1" --keychain-profile "$PROFILE" --wait 2>&1 | tee "$log" | sed 's/^/  /' \
       || ! grep -q "status: Accepted" "$log"; then
        local id; id=$(awk '/^  id:/{print $2; exit}' "$log")
        echo "  ✗ notarization failed. Details: xcrun notarytool log $id --keychain-profile $PROFILE"
        exit 1
    fi
}

mkdir -p "$DIST"
if [ "${SKIP_NOTARIZE:-0}" != "1" ]; then
    echo "▸ Notarizing the app"
    ZIP="$DIST/Nostyra-notarize.zip"
    ditto -c -k --keepParent "$APP" "$ZIP"   # ditto keeps the signature intact
    notarize "$ZIP"
    rm -f "$ZIP"
    xcrun stapler staple "$APP" | sed 's/^/  /'
fi

echo "▸ Packing DMG"
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "Nostyra $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
rm -rf "$STAGE"
codesign --force --timestamp --sign "$DEV_ID" "$DMG"

if [ "${SKIP_NOTARIZE:-0}" != "1" ]; then
    echo "▸ Notarizing the DMG"
    notarize "$DMG"
    xcrun stapler staple "$DMG" | sed 's/^/  /'
    spctl --assess --type open --context context:primary-signature -v "$DMG" 2>&1 | sed 's/^/  /'
fi
(cd "$DIST" && shasum -a 256 "$(basename "$DMG")" > "$(basename "$DMG").sha256")
echo "✓ $DMG"
