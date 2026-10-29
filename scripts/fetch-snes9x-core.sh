#!/usr/bin/env bash
# Downloads the nightly snes9x_libretro core for the current OS/arch from the
# libretro buildbot into ./cores/. Used by local dev and CI.
# The core is an unpinned nightly, so we print its SHA256 for traceability
# instead of verifying against a golden checksum.
set -euo pipefail

# Single place to change if the buildbot layout moves.
BUILDBOT_BASE="https://buildbot.libretro.com/nightly"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest_dir="${repo_root}/cores"

os="$(uname -s)"
arch="$(uname -m)"
case "${os}" in
  Linux)
    [[ "${arch}" == "x86_64" ]] || { echo "error: unsupported Linux arch ${arch}" >&2; exit 1; }
    platform="linux/x86_64"; lib="snes9x_libretro.so" ;;
  Darwin)
    case "${arch}" in
      arm64) platform="apple/osx/arm64" ;;
      x86_64) platform="apple/osx/x86_64" ;;
      *) echo "error: unsupported macOS arch ${arch}" >&2; exit 1 ;;
    esac
    lib="snes9x_libretro.dylib" ;;
  MINGW*|MSYS*|CYGWIN*)
    platform="windows/x86_64"; lib="snes9x_libretro.dll" ;;
  *) echo "error: unsupported OS ${os}" >&2; exit 1 ;;
esac

url="${BUILDBOT_BASE}/${platform}/latest/${lib}.zip"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

echo "Fetching ${url}"
curl --proto '=https' --tlsv1.2 -fsSL --retry 3 "${url}" -o "${tmp_dir}/core.zip"
unzip -oq "${tmp_dir}/core.zip" -d "${tmp_dir}/out"
[[ -f "${tmp_dir}/out/${lib}" ]] || { echo "error: ${lib} not found in archive" >&2; exit 1; }

mkdir -p "${dest_dir}"
mv -f "${tmp_dir}/out/${lib}" "${dest_dir}/${lib}"

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "${dest_dir}/${lib}"
else
  shasum -a 256 "${dest_dir}/${lib}"
fi
echo "Core ready: ${dest_dir}/${lib}"
