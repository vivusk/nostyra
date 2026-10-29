#!/usr/bin/env bash
# Refreshes the bundled identification DATs and the five libretro-database metadat
# attribute DATs (developer, publisher, genre, releaseyear, maxusers) of every
# system from libretro-database at a pinned commit. Manual use only: bump
# DAT_COMMIT, run, review the diff, set METADATA_SOURCE_VERSION in
# crates/game-library/src/metadata_dat_index.rs to the same commit (a test checks it),
# commit. The data is CC-BY-SA-4.0 (libretro-database); DAT data from No-Intro
# (cartridges) and Redump (discs).
# DAT_ENTRIES = "<source folder under metadat/>/<name>"; <name> mirrors
# `SystemSpec.dat_file` in crates/system-catalog (a test checks it; each system's
# metadat files use the same name). Vendored flat: third-party/libretro-database/<name>.
# Redump systems: metadat lines "<tab>description …" (long plot summaries, unused)
# are dropped; an attribute file missing upstream becomes a header-only placeholder
# so the metadata index stays complete.
set -euo pipefail

DAT_COMMIT="9a899f5b13ed1c355491e342203be29bc573323b"
DAT_ENTRIES=(
  "no-intro/Nintendo - Super Nintendo Entertainment System.dat"
  "no-intro/Nintendo - Nintendo Entertainment System.dat"
  "no-intro/Nintendo - Game Boy.dat"
  "no-intro/Nintendo - Game Boy Color.dat"
  "no-intro/Nintendo - Game Boy Advance.dat"
  "no-intro/Nintendo - Nintendo 64.dat"
  "no-intro/Nintendo - Nintendo DS.dat"
  "no-intro/Sega - Mega Drive - Genesis.dat"
  "no-intro/Sega - Master System - Mark III.dat"
  "no-intro/Sega - Game Gear.dat"
  "no-intro/Sega - SG-1000.dat"
  "redump/Sony - PlayStation.dat"
  "redump/Sega - Dreamcast.dat"
  "redump/Sony - PlayStation Portable.dat"
  "redump/Nintendo - GameCube.dat"
  "redump/Nintendo - Wii.dat"
)
METADAT_ATTRS=(developer publisher genre releaseyear maxusers)
REPO_RAW="https://raw.githubusercontent.com/libretro/libretro-database/${DAT_COMMIT}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest_dir="${repo_root}/third-party/libretro-database"
mkdir -p "${dest_dir}"

fetch() {
  curl --proto '=https' --tlsv1.2 -fsSL --retry 3 "$1" -o "$2"
}

placeholder() { # <system name> <attribute> <file>
  printf 'clrmamepro (\n\tname "%s"\n\tdescription "Header only: libretro-database has no %s metadat for this system"\n)\n' \
    "$1" "$2" > "$3"
}

for entry in "${DAT_ENTRIES[@]}"; do
  source="${entry%%/*}"
  dat_name="${entry#*/}"
  fetch "${REPO_RAW}/metadat/${source}/${dat_name// /%20}" "${dest_dir}/${dat_name}"
  for attr in "${METADAT_ATTRS[@]}"; do
    mkdir -p "${dest_dir}/metadat/${attr}"
    out="${dest_dir}/metadat/${attr}/${dat_name}"
    if ! fetch "${REPO_RAW}/metadat/${attr}/${dat_name// /%20}" "${out}"; then
      echo "note: no metadat/${attr}/${dat_name} upstream; writing a placeholder" >&2
      placeholder "${dat_name%.dat}" "${attr}" "${out}"
    elif [[ "${source}" == "redump" ]]; then
      sed -i '/^\tdescription /d' "${out}"
    fi
  done
done
fetch "${REPO_RAW}/LICENSE" "${dest_dir}/LICENSE"

{
  echo "Source:  https://github.com/libretro/libretro-database"
  echo "Commit:  ${DAT_COMMIT}"
  echo "Files (one set per system, stored as metadat/<source>/<name> as <name>,"
  echo "       metadat/<attribute>/<name> as metadat/<attribute>/<name>):"
  for entry in "${DAT_ENTRIES[@]}"; do
    echo "         ${entry}"
  done
  echo "         attributes: developer, publisher, genre, releaseyear, maxusers"
  echo "         redump metadat: \"description\" lines removed; attributes missing"
  echo "         upstream are header-only placeholders."
  echo "License: CC-BY-SA-4.0 (see LICENSE). Game data by No-Intro (https://no-intro.org)"
  echo "         and Redump (http://redump.org), compiled by the libretro-database"
  echo "         contributors."
  echo "Use:     bundled data files (unmodified except as noted above). The DATs identify"
  echo "         dumps by hash (discs also by serial); the metadat files give developer,"
  echo "         publisher, genre, year and players."
  echo "         Refresh with scripts/update-libretro-dat.sh (or .ps1)."
} > "${dest_dir}/SOURCE.txt"

echo "DATs + metadat updated: ${dest_dir} @ ${DAT_COMMIT}"
