#!/usr/bin/env bash
# Refreshes crates/artwork-cache/tests/fixtures/snes-boxart-names.txt: the list
# of SNES Named_Boxarts file names (without ".png") in libretro-thumbnails, used
# by the offline boxart coverage test. Manual use only: run, review the diff,
# commit. Uses the unauthenticated GitHub API (60 requests/hour, 2 per run).
# File names only; no artwork is downloaded or committed.
set -euo pipefail

REPO_API="https://api.github.com/repos/libretro-thumbnails/Nintendo_-_Super_Nintendo_Entertainment_System"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest="${repo_root}/crates/artwork-cache/tests/fixtures/snes-boxart-names.txt"
tmp="$(mktemp)"
trap 'rm -f "${tmp}"' EXIT

fetch() {
  curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -H "Accept: application/vnd.github+json" "$1"
}

# First "sha" of the commit object is the commit itself.
commit_json="$(fetch "${REPO_API}/commits/master")"
commit="$(grep -m1 -o '"sha": *"[0-9a-f]\{40\}"' <<< "${commit_json}" | grep -o '[0-9a-f]\{40\}')"
fetch "${REPO_API}/git/trees/${commit}?recursive=1" > "${tmp}"
grep -q '"truncated": *false' "${tmp}" || { echo "error: GitHub tree listing is truncated" >&2; exit 1; }

{
  echo "# libretro-thumbnails SNES Named_Boxarts snapshot $(date -u +%Y-%m-%d) commit ${commit}"
  # Paths look like "Named_Boxarts/<name>.png"; sanitised names hold no quotes.
  sed -n 's|^ *"path": *"Named_Boxarts/\([^/"]*\)\.png",*$|\1|p' "${tmp}" | LC_ALL=C sort -u
} > "${dest}"

echo "wrote $(($(wc -l < "${dest}") - 1)) names to ${dest}"
