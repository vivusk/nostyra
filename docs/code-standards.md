# Nostyra — Code Standards

Last updated: 2026-10-29

## Principles
- YAGNI · KISS · DRY. Build only what the current milestone needs.
- Files < 200 lines; split by responsibility.
- File names: kebab-case, self-describing (Rust modules use snake_case per language rules: `frame_converter.rs`; crates/dirs use kebab-case: `libretro-host`).

## Rust
- Edition 2021+, stable toolchain pinned in `rust-toolchain.toml`.
- `cargo fmt` + `cargo clippy -- -D warnings` must pass in CI.
- Errors: `thiserror` in library crates, `anyhow` in binaries. No `unwrap()`/`expect()` outside tests and truly unreachable cases (comment why).
- `unsafe` only inside `libretro-host` FFI boundary, the `hw-render-gl` OpenGL backend (glow calls, SDL GL symbol lookup for cores) and the `achievements` rcheevos FFI (its private `ffi` module only: declarations, `#[repr(C)]` mirrors checked by C size/offset probe tests, rc_client trampolines, safe wrappers), plus direct `sdl3::sys` / OS calls the safe wrappers lack (keep each in one small fn with an item-level `#[allow(unsafe_code)]`); every `unsafe` block has a `// SAFETY:` comment. Every other crate / app root carries `#![deny(unsafe_code)]`, so the build enforces the rule.
- Never let a Rust panic unwind across `extern "C"` callbacks — wrap callback bodies in `std::panic::catch_unwind`.
- Logging: `tracing`; no `println!` in library crates.
- Settings that both apps read go through `config-layers` and paths keyed by ROM go through `ipc_protocol` (`rom_key`, `game_folder_key`, `saves_dir_for_game`, `game_config_path_for_game`, `state_file` / `StateSlotId`, `SRAM_FILE_NAME`): one owner per rule.
- Input / action / hotkey names, pad labels, reserved inputs and default bindings live only in `config-layers::input_binding_names`; binding conflicts are resolved only by `config_layers::resolve_input_conflicts`. `input-mapping` converts names to winit / SDL types (parity test: every name converts and round-trips); the launcher UI gets the lists from `get_input_catalog`, never a hand-kept TS copy.
- System and core facts (ids, extensions, size limits, header rules, DAT / thumbnail names, pad actions, per-system pad button names, core per system) live only in `crates/system-catalog`; every other crate and the UI (`get_systems`, `get_setup_state.cores`) read them. `scripts/check-system-literals.*` (CI) rejects `"snes"` / `"nes"` / `"snes9x"` / `"mesen"` / `".sfc"`-style literals in non-test code elsewhere (allow-list inside the script).
- Core quirks are registry flags, never `if core == …` in host or runner code: `CoreSpec.hw_render_required` (a refused GL request is fatal; states need the hw context), `strict_state_version`, `keep_library_loaded` (the runner then ends the process with `process_hard_exit::exit_now` after all saves and reporting), `requery_state_size`, `exact_state_size`, `port_devices`, `process_env`.
- Save-state files are loaded only through the runner's `StateFileGuard` (name, version, content tag, cap, length, CRC before the core); the launcher's verdict comes from the same `ipc_protocol::state_incompatibility`.
- Core process environment (`CoreSpec.process_env`) is set only through `libretro_host::set_process_env` (it also updates msvcrt's copy on Windows, which MinGW-built cores read); never call `std::env::set_var` for a core directly.
- RetroAchievements (M15): every `rc_client` call goes through `achievements::AchievementClient` on the session thread (`!Send`); HTTP work leaves that thread only as `Send` data (`ServerRequest`, background jobs) and its results come back through the client's drain; never hand a C callback or `cb_data` to another thread. Credentials, journal, snapshot and badge paths have one owner each in `crates/achievements` (`credentials_path`, `PendingJournal`, `snapshot_path`, `badge_path`; badge / icon names validated). Hardcore visibility is the single const `achievements::HARDCORE_UI_ENABLED` read through `hardcore_ui_enabled()`; core-option rules per core are registry data (`CoreSpec.hardcore_required_options`), never `if core == …`.
- Overlay text (M15): Unicode text (achievement titles, descriptions, user names) is drawn only with `video-renderer::TtfText` (embedded Noto Sans, NFC-normalised, cached glyphs, `.notdef` box for missing glyphs); the 5×7 bitmap font stays for ASCII-only status and menu labels. Fonts are loaded lazily on the first achievement overlay need, never at runner start; a new font must ship with its licence text in `assets/fonts/` and in the portable `licenses\` folder.
- Files written while a game may run (SRAM, states, thumbnails, config layers) use `config_layers::write_atomically`.
- Hot paths (per-frame/per-sample): no allocation, no locks contended with UI thread.
- Doc comments (`///`) on all public items; explain *why*, not *what*.

## TypeScript / React (M1+)
- Strict TS, function components, hooks.
- Components < 200 lines; one component per file, kebab-case filenames (`game-cover-card.tsx`).
- Server state via TanStack Query over Tauri commands; no ad-hoc global stores unless needed.
- Tailwind for styling; design tokens in one theme file.
- All interactive elements must be focusable (keyboard now; Big Picture spatial navigation later — deferred, Later).
- Routing: state-based route stack in `src/navigation/` (`navigate` replaces the root for sidebar
  pages, pushes Detail; `back`); no router library. Page-level view state that must survive Back
  (Library search/sort/filter/scroll) lives in the navigation provider.
- Images: game art goes through `CoverImage` (tile colour + monogram fallback, `object-fit:
  contain` for covers, lazy + fade) and `assetUrl()`; only backend-provided absolute paths reach
  `convertFileSrc`, never user input.
- Long lists: virtualise (`@tanstack/react-virtual`) with fixed row heights; memoise row items and
  keep callbacks stable.

## Testing
- Unit tests next to code (`#[cfg(test)]`).
- Integration tests in `crates/*/tests/`. Emulation tests use `test-assets/roms/nostyra-test.{sfc,nes,gb,gbc,gba,gen,sms,gg,sg,z64}` the PlayStation test discs `nostyra-test-psx*.{cue,bin,m3u}`, the Dreamcast test program and discs `nostyra-test-dc*.{elf,gdi,cue,m3u}` and the PSP test program and containers `nostyra-test-psp.{elf,iso,cso}` + `nostyra-test-psp/EBOOT.PBP` (the PSP DVD CHD is built in test code) the DS test ROMs `nostyra-test.nds` + `nostyra-test-save.nds` (the encrypted-dump fixture is built in test code; never Nintendo logo bytes) and the GameCube / Wii test programs `nostyra-test.dol` + `nostyra-test-wii.dol` (GC/Wii disc containers are header-only fixtures built in test code, `disc-image` `test_nintendo_images.rs`), all self-written, GPLv3, rebuilt reproducibly by Python scripts (see README.txt). Never commercial ROMs, discs or BIOS files in repo.
- Frame-hash smoke test: run N frames headless, assert frame checksum stable.
- One libretro core session at a time per process: in-binary real-core checks run inside one `#[test]` fn per core (`runner_real_core_suite`, `runner_real_core_suite_mesen`, …, `runner_real_core_suite_mupen64plus_next`) under a shared lock; separate test binaries are separate processes. Per-core quirks are explicit (`RunAheadExpectations`), never silently skipped.
- OpenGL tests (M10) go through one gate (`gl_test_gate` in `real_core_session_loader.rs` and `tests/support`, built on `hw_render_gl::probe_gl_context`; keep the two copies equal): without a GL 3.3 context they skip with a note, `NOSTYRA_REQUIRE_GL=1` turns that skip into a failure (Linux CI on Mesa llvmpipe, and the local gate), and `NOSTYRA_FORCE_NO_GL=1` makes the runner's context creation fail and the gates skip, so `cargo test --workspace` simulates a PC without OpenGL (the env hook is read only in debug builds). In-binary GL tests skip on macOS (SDL windows are main-thread only). A core that renders on the GPU also gets a software-renderer variant when it has one (Mupen: Angrylion via `--core-option`; PPSSPP: `ppsspp_backend=none`, plus one GL smoke; tests only), so CI without GL still covers core, saves, input, audio and states. Exception: Dolphin's states crash without a hw context, so its Null-renderer suite (`dolphin_renderer=Null`) covers boot, memory, pad and audio only and every Dolphin state suite is GL-gated (none on Windows / macOS CI). `NOSTYRA_FORCE_NO_GL=refuse` refuses the `SET_HW_RENDER` request itself (for cores that survive a refusal). Process tests that spawn a runner which might hang use `support::output_within` (deadline, kill, clear panic) instead of `output()`.
- Large unit-test modules live in a sibling `*_tests.rs` file (`#[cfg(test)] #[path = "…_tests.rs"] mod tests;`) to keep the code file < 200 lines. A module with children uses `name.rs` + `name/` (not `name/mod.rs`).
- Launcher/runner shared rules get a parity test against the real runner binary (`--print-config`); tests that need a core or the built runner skip with a note when it is missing.
- RetroAchievements tests never touch the network: in-process tests use `FakeTransport` with `RequestDispatch::Inline`; process tests use the debug-only `NOSTYRA_RA_FAKE_SERVER=<dir>` plus `NOSTYRA_OFFLINE=1`. Per-system fake sets live in `test-assets/achievements/` and are regenerated by `generate-fake-sets.py` (CI checks the output is unchanged); a test ROM change that moves its hash must regenerate them. Hardcore paths are tested by passing `hardcore_ui` in-process or `NOSTYRA_RA_HARDCORE_UI=1` for the runner binary, never by flipping the const.
- Consumers of runner stdout (`--print-config`, `--probe-core`, `--ipc`) parse one JSON value starting at `{`: cores print unterminated C stdio text around it.

## Git
- Conventional commits (`feat:`, `fix:`, `docs:`…).
- No ROMs, BIOS, or copyrighted assets committed.
