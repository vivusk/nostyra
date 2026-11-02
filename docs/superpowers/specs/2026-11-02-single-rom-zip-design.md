# Single-ROM ZIP support

Status: approved by the user on 2026-11-02. One ROM per ZIP, limited to NES,
SNES, Sega and GBA; implement the recommended approach below.

## Intent and scope

Add an optional library scan of ZIP files and allow the resulting games to play
without manual extraction. Supported systems: `nes`, `snes`, `gba`, and the
existing Sega cartridge systems (Mega Drive/Genesis, Master System, Game Gear,
SG-1000). Exclude other systems, disc games, nested archives and multiple ROMs.

## Approaches

1. **Recommended:** inspect and hash the single ROM in memory during scan; extract
   that ROM to a temporary file when playing. Store the ZIP path as `rom_path`,
   with the archive's size/mtime for rescan caching and the ROM's canonical hashes
   for DAT matching. This works with existing cores and avoids persistent copies.
2. Extract archives permanently into the library before scanning. This reuses the
   current scanner but creates extra user-visible ROM files and consumes disk.
3. Pass ZIP files directly to cores. This relies on individual core support and
   does not give scan and playback a consistent archive-selection policy.

## Proposed behavior

- Add **Scan ZIP archives** under Settings → ROM folders, off by default and
  persisted in the existing settings table. Changing it schedules a rescan.
- Match `.zip` and ROM extensions without case sensitivity. Permit a ROM in a
  subdirectory and accompanying non-ROM files such as a README. Ignore directory
  and macOS metadata entries. Require exactly one ROM; reject ambiguous archives.
- Classify the decompressed ROM using existing extension, size and content checks.
  Use the same header-aware hashing and DAT matching as uncompressed games. Use
  the member filename as the unmatched display title.
- Keep the archive path as the library/session identity. Existing save and config
  path calculations use that stable path and the canonical ROM content hash,
  never a temporary extraction path. Reveal in folder opens the ZIP location.
- Disabling ZIP scanning stops discovering/updating archived games, but preserves
  existing archived rows and their playability; a removed archive is still marked
  missing. Enabling again resumes normal scanning.
- Play validates the archive again, enforces the selected system, and extracts
  only its selected ROM into an owned temporary directory for the session. The
  core and RetroAchievements read the extracted ROM. Cleanup follows session
  lifetime, including error paths. Never overwrite the source archive.
- Use bounded decompression according to supported systems' size limits; reject
  unsafe member paths, encrypted/unsupported compression, corruption and archives
  with no playable ROM. Report errors through existing scan/game error surfaces.
- Existing uncompressed ROM behavior remains covered by regression checks.

## Constraints and remaining implementation choices

Reuse the workspace `zip` dependency, registry classification, database settings,
and runner staging conventions. No database migration is required for one ROM per
archive. The implementer chooses helper names and temporary-directory ownership,
and verifies soft-patch lookup and state checks against the original ZIP identity.

## Validation

Exercise valid archives across every allowed system, canonical hash/DAT parity,
multiple/unsupported ROM rejection, size/corruption/path failures, rescans and
missing archives with the option on/off, and runner identity/extraction cleanup.
Run the relevant Rust suites, workspace checks as available, and launcher
TypeScript typecheck/build. Report unavailable native prerequisites explicitly.
