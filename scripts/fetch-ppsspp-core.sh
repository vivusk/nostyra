#!/usr/bin/env bash
# Downloads the nightly ppsspp_libretro core for the current OS/arch from the
# libretro buildbot into ./cores/, plus the PPSSPP system files
# (assets/system/PPSSPP.zip) into ./cores/PPSSPP/ (the runner copies them to
# <data>/system/PPSSPP/). Used by local dev and CI.
# Both are unpinned nightlies, so we print their SHA256 for traceability
# instead of verifying against a golden checksum.
set -euo pipefail

# Single place to change if the buildbot layout moves (core-manager mirrors both roots).
BUILDBOT_BASE="https://buildbot.libretro.com/nightly"
BUILDBOT_ASSETS_ROOT="https://buildbot.libretro.com"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest_dir="${repo_root}/cores"

os="$(uname -s)"
arch="$(uname -m)"
case "${os}" in
  Linux)
    [[ "${arch}" == "x86_64" ]] || { echo "error: unsupported Linux arch ${arch}" >&2; exit 1; }
    platform="linux/x86_64"; lib="ppsspp_libretro.so" ;;
  Darwin)
    case "${arch}" in
      arm64) platform="apple/osx/arm64" ;;
      x86_64) platform="apple/osx/x86_64" ;;
      *) echo "error: unsupported macOS arch ${arch}" >&2; exit 1 ;;
    esac
    lib="ppsspp_libretro.dylib" ;;
  MINGW*|MSYS*|CYGWIN*)
    platform="windows/x86_64"; lib="ppsspp_libretro.dll" ;;
  *) echo "error: unsupported OS ${os}" >&2; exit 1 ;;
esac

url="${BUILDBOT_BASE}/${platform}/latest/${lib}.zip"
assets_url="${BUILDBOT_ASSETS_ROOT}/assets/system/PPSSPP.zip"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

echo "Fetching ${url}"
curl --proto '=https' --tlsv1.2 -fsSL --retry 3 "${url}" -o "${tmp_dir}/core.zip"
unzip -oq "${tmp_dir}/core.zip" -d "${tmp_dir}/out"
[[ -f "${tmp_dir}/out/${lib}" ]] || { echo "error: ${lib} not found in archive" >&2; exit 1; }

echo "Fetching ${assets_url}"
curl --proto '=https' --tlsv1.2 -fsSL --retry 3 "${assets_url}" -o "${tmp_dir}/assets.zip"
unzip -oq "${tmp_dir}/assets.zip" -d "${tmp_dir}/assets"
[[ -f "${tmp_dir}/assets/PPSSPP/compat.ini" ]] || { echo "error: PPSSPP/compat.ini not found in archive" >&2; exit 1; }

mkdir -p "${dest_dir}"
mv -f "${tmp_dir}/out/${lib}" "${dest_dir}/${lib}"
rm -rf "${dest_dir}/PPSSPP"
mv "${tmp_dir}/assets/PPSSPP" "${dest_dir}/PPSSPP"

sha() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi
}
sha "${dest_dir}/${lib}"
sha "${tmp_dir}/assets.zip"
echo "Core ready: ${dest_dir}/${lib}"
echo "System files ready: ${dest_dir}/PPSSPP"
