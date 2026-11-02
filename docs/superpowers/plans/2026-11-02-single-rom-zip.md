# Single-ROM ZIP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Scan and play an optional ZIP containing one NES, SNES, Sega cartridge or GBA ROM.

**Architecture:** Keep the archive path as library/session identity. A shared game-library helper validates and reads its single member; the runner owns a temporary extraction for the core while saves/config/session reporting retain the original path.

**Tech Stack:** Rust, existing zip dependency, SQLite settings, Tauri, React/TanStack Query.

**Spec:** `docs/superpowers/specs/2026-11-02-single-rom-zip-design.md` (approved).

## Global Constraints

- One ROM per ZIP; NES, SNES, GBA, Mega Drive/Genesis, Master System, Game Gear, SG-1000 only.
- Scan ZIP archives is off by default; source ZIPs remain unchanged.
- ROM bytes supply canonical hashes; archive path supplies stable session identity.
- No new dependency version and no schema migration.

## Review Focus

- A ZIP replaced since scanning must be revalidated at Play, including selected system.
- A nested member must never write outside its temporary extraction directory.
- Disabling scanning must preserve existing entries without hiding actually removed archives.
- Archive filenames may differ from member filenames; titles and save identity have distinct purposes.
- Failed launch and successful session teardown must both release temporary content.

### Task 1: Shared archive reader and library scan

**Files:** Create `crates/game-library/src/rom_archive.rs` and `crates/game-library/tests/zip_scan_tests.rs`; modify `Cargo.toml` dependencies within game-library, `src/lib.rs`, `src/folder_scanner.rs`, `src/scanned_game_builder.rs`, `src/game_content_sha1.rs`, `src/library_scan_discovery.rs`, and `src/library_scan_job.rs`.

**Interfaces:** Produce a shared archive reader returning member filename, classified system and bounded ROM bytes. Add an explicit scan option while retaining existing scan entry points for callers/tests. Consumers store the ZIP path and archive fingerprint, with member-content hashes and title.

- [x] Write and run failing tests for supported-system discovery, canonical hash/DAT parity, member title and archive path/fingerprint.
- [x] Write rejection cases for multiple/unsupported ROMs, metadata entries, corrupt archives, unsafe paths and oversized decompression.
- [x] Implement the bounded reader with the existing workspace zip dependency and registry probes; wire scan/build/hash paths to it.
- [x] Test rescan caching, changed archives, absent archives and preservation while ZIP scanning is disabled.
- [x] Run `cargo test -p game-library`; inspect existing scan/lifecycle regression coverage.

### Task 2: Runner preparation and stable identity

**Files:** Create `apps/runner/src/archived_content.rs`; modify `apps/runner/src/main.rs`, `src/loaded_session_run.rs`, `src/runner_exit_codes.rs`, and `Cargo.toml` if a temporary-directory dependency is needed. Reuse `runner_data_paths`, `session_reporter` and staged-content conventions.

**Interfaces:** Consume Task 1's validated archive reader. Produce a prepared playable ROM path owned for the full session, alongside the original identity path used for saves, config and session reporting.

- [x] Add failing tests for extraction bytes, selected-system mismatch after ZIP replacement, distinct identity/playback paths, and cleanup after success/error.
- [x] Prepare archived ROMs before core loading; make validation errors map to the existing ROM-load failure code.
- [x] Keep config, saves, pending-session reporting and patch/state checks aligned with the original game identity; pass playable content to the core and achievements.
- [x] Run runner tests and existing runner-session tests; add a headless archive launch check if a usable test core is available.

### Task 3: Persisted option and launcher UI

**Files:** Modify `apps/launcher/src-tauri/src/commands/scan_commands.rs`, `src/lib.rs`, `apps/launcher/src/api/tauri-commands.ts`, `src/api/query-keys.ts`, `src/hooks/use-library-queries.ts`, and `src/pages/settings-page.tsx`; add a focused settings component if needed. Update README archive support notes.

**Interfaces:** Add get/set commands for persisted ZIP scan opt-in, read it at scan start and pass Task 1's option. Expose the option via typed commands and query/mutation hooks.

- [x] Add failing backend tests for default-off behavior and persisted option values.
- [x] Register get/set commands and wire scans to the persisted value; schedule rescan on change using the existing queue behavior.
- [x] Add Scan ZIP archives checkbox with supported-system text and mutation/error states.
- [x] Run launcher backend checks, `npm run typecheck`, `npm run build`, and `cargo fmt --all -- --check`.
- [x] Run workspace tests as native prerequisites permit; inspect the diff against the spec and record any unavailable verification.

## Verification results

- `cargo test -p game-library -p runner-session`: passed, including ZIP scan and settings tests.
- `cargo test -p nostyra-runner --test runner_zip_tests`: 4 passed, including real-core playback parity for all seven supported systems, identity, patches and cleanup.
- `cargo test -p nostyra-launcher --lib`: 135 passed, 2 ignored.
- Launcher `npm run typecheck` and `npm run build`: passed.
- Independent read-only code review found no actionable issues.
- `cargo test --workspace --no-fail-fast`: four failures, all reproduced on an unmodified HEAD checkout with the same environment and cores. `ipc-protocol::shared_data_paths::tests::default_root_ends_in_nostyra_data_dir` expects uppercase `Nostyra` although Linux provides lowercase `nostyra`. The `achievements_real_core_gg`, `achievements_real_core_sg1000` and `achievements_real_core_sms` tests expect hardcore unlock at frame 20; the downloaded nightly Genesis Plus GX core produces frame 21. The targeted original-HEAD runs failed identically.

Implementation choices: work is on `feat/single-rom-zip` in the original checkout; old scan entry points remain ZIP-disabled by default. Unidentified ZIPs may be re-read for DAT rematching because stored size fingerprints describe the archive; Play re-hashes extracted bytes so replacing an archive cannot use the prior game's save identity.
