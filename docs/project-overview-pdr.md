# Nostyra — Project Overview & PDR

> **Old Worlds. New Ways.**

Last updated: 2026-10-29 · Status: M0–M4 + M6–M15 implemented (Windows; manual checks pending); M5 HD Remaster lives on branch `hd-remaster`; Big Picture & Deck deferred

## 1. Vision

Nostyra is a free, open-source, cross-platform retro game launcher + emulation frontend with a modern, Epic Games Store–style UI. It manages a game library (artwork, metadata, playtime, saves) and runs games through libretro cores. SNES (Snes9x), NES (Mesen), Game Boy / Game Boy Color / Game Boy Advance (mGBA), PlayStation (PCSX ReARMed), Sega Mega Drive / Master System / Game Gear / SG-1000 (Genesis Plus GX), Nintendo 64 (Mupen64Plus-Next, OpenGL), Sega Dreamcast (Flycast, OpenGL), Sony PSP (PPSSPP, OpenGL), Nintendo DS (melonDS DS, two screens, touch via the mouse) and Nintendo GameCube / Wii (Dolphin, OpenGL, Wii Remote pointer via the mouse) today; one library for every system, each ROM classified by its content, new systems added as registry data; RetroAchievements (softcore) on 14 of them.

## 2. Goals

| # | Goal | Measure |
|---|------|---------|
| G1 | Modern, polished library UX (Epic style) | Library of 1,000 games scrolls at 60 fps; cold start < 1.5 s |
| G2 | Low-latency, smooth emulation | No audio crackle / frame stutter on 60 Hz and VRR displays; run-ahead available |
| G3 | Cross-platform | Windows 10+, macOS 12+ (arm64 + x64), Linux (x64), Steam Deck |
| G4 | Controller-first option | Big Picture mode fully navigable by gamepad (deferred to Later, 2026-10-18) |
| G5 | Multi-system ready | Adding a new system = add core + platform data, no core-specific code |

## 3. Non-Goals

- No ROM distribution, ROM download, or piracy links.
- No custom emulation core (we host libretro cores).
- No mobile / web / console ports.
- No monetization, accounts, or telemetry by default.

## 4. Target Users

- Retro players who want a "Steam/Epic for my ROM collection".
- Steam Deck owners wanting a clean, controller-friendly launcher (deferred, Later).
- Collectors who care about correct metadata and artwork.

## 5. Key Decisions

| Topic | Decision | Rationale |
|-------|----------|-----------|
| License | GPLv3 | Compatible with GPL cores (Mesen, bsnes); open source |
| Language | Rust (backend + runner), TypeScript (UI) | Performance, safety, cross-platform |
| Shell | Tauri 2 | Small footprint vs Electron; Rust backend |
| UI | React + TS + Vite + Tailwind | Large ecosystem, team preference |
| Emulation | libretro API host; Snes9x (SNES), Mesen (NES), mGBA (GB, GBC, GBA), PCSX ReARMed (PS1), Genesis Plus GX (MD, SMS, GG, SG-1000), Mupen64Plus-Next (N64), Flycast (Dreamcast), PPSSPP (PSP), melonDS DS (NDS), Dolphin (GameCube, Wii) | Swappable cores per system (registry), multi-system path |
| Process model | Launcher and runner are separate processes | Crash isolation, latency, CLI launch |
| Video | winit + wgpu + librashader | Vulkan/Metal/DX12, RetroArch slang shaders |
| GPU cores | Backend-agnostic hw-render API in `libretro-host`; OpenGL backend `hw-render-gl` (hidden SDL3 context, FBO readback into the wgpu presenter) | Shaders / overlay / thumbnails unchanged; Vulkan and zero-copy plug in later |
| Audio/Input | SDL3 | Unified gamepad DB, Steam Input friendly |
| Storage | SQLite (rusqlite, WAL) | Embedded, fast, zero setup |
| Achievements | rcheevos v12.5.0 (MIT) vendored unmodified, `rc_client` in the runner behind a safe single-thread wrapper; softcore default, hardcore coded but hidden until RA approves the client | Same library and data as RetroArch; RA requires ≥ 6 months public before hardcore credit |

### Snes9x license note
Snes9x has a non-commercial, non-GPL license. Nostyra (GPLv3) does **not** link Snes9x source; it loads the `snes9x_libretro` core as a separate dynamic library at runtime (same model as RetroArch). Nostyra stays free; no commercial use of Snes9x. The NES core `mesen_libretro` (Mesen 0.9.9) is GPLv3, downloaded and loaded the same way. Mesen2 was the first choice but its buildbot build rejected every ROM (M6 spike); switching later is a registry entry. The handheld core `mgba_libretro` (MPL-2.0, file-level copyleft, GPL-compatible) is downloaded and loaded the same way; no BIOS is required or shipped. The PlayStation core `pcsx_rearmed_libretro` (GPL-2.0-or-later) is downloaded and loaded the same way. The Sega core `genesis_plus_gx_libretro` has a non-commercial licence like Snes9x: downloaded at runtime from the buildbot, never bundled or shipped in a paid distribution. The Nintendo 64 core `mupen64plus_next_libretro` and the Dreamcast core `flycast_libretro` and the PSP core `ppsspp_libretro` (all GPL-2.0-or-later) are downloaded and loaded the same way; PPSSPP's system files (`PPSSPP.zip`: fonts, dialog atlas, compat table) are downloaded from the buildbot with it, never bundled. The Nintendo DS core `melondsds_libretro` (melonDS DS, GPL-3.0-or-later) is downloaded and loaded the same way; it runs on its built-in FreeBIOS and generated firmware, so only decrypted dumps play. The GameCube / Wii core `dolphin_libretro` (Dolphin, GPL-2.0-or-later) is downloaded and loaded the same way, with its system files (`Dolphin.zip`) from the buildbot, never bundled; it boots with its built-in IPL replacement and Wii keys. **BIOS policy**: Nostyra never bundles, downloads or links BIOS files; PS1, Dreamcast, DS, GameCube and Wii games run on the core's built-in BIOS replacement, and a BIOS the user dumped from their own console can be added from Settings → BIOS (a Dreamcast BIOS only with a known checksum).

## 6. Functional Requirements (by milestone)

- **M0 Runner spike**: load libretro core, play ROM with video, audio, gamepad/keyboard, save state, SRAM.
- **M1 Library**: folder scan, CRC32/SHA1 hashing, No-Intro DAT match, SQLite, grid view, launch runner.
- **M2 Metadata & UI**: offline metadata (vendored libretro-database metadat), cover/hero art from libretro-thumbnails (runtime download, local cache), Epic-style Home/Library/Detail, favorites, metadata editing. Online scrapers (ScreenScraper / IGDB) moved to the backlog.
- **M3 Play features**: save-state manager with thumbnails (in-game menu + launcher Detail tab), layered TOML config (global → system → core → game, launcher edits global + game), bundled CRT slang presets via librashader, single-instance run-ahead 0–2, hold-to-rewind, pad-navigable pause menu. Input remapping moved to M4.
- **M4 Controls & Resume**: every SNES button (12) and runner hotkey (8) rebindable for keyboard and gamepad, global with per-game overrides, edited in the launcher (Settings → Controls, Detail → Settings → Controls; keyboard capture in the page, gamepad capture via SDL in the launcher backend); Esc, pad Guide and the Select+Start hold always open the pause menu; pause-menu "Reset game" (core soft reset, SRAM kept); resume point saved on every clean exit (`auto` pseudo-slot) and offered on the next Play (Continue / Start fresh, mode `ask` / `always` / `never`, warnings for a newer in-game save, a changed patch or core version).
- **M5 HD Remaster**: developed on branch `hd-remaster` (not on main).
- **M8 PlayStation**: first disc system — PS1 on PCSX ReARMed (`.cue` + tracks, `.bin`, `.iso`, `.chd`, `.pbp`, multi-disc `.m3u`), disc content probe + multi-file grouping, streaming hash + serial identification (Redump DAT), disc swap from the pause menu, L2/R2, BIOS settings (HLE default), memory card import, scan problems dialog.
- **M9 Sega 8/16-bit**: Mega Drive / Genesis, Master System, Game Gear and SG-1000 on one Genesis Plus GX core (registry data: four systems, one core, DATs, migration); `.md` / `.bin` decided by the `SEGA` header (Markdown files ignored), `.smd` hashed as the raw dump, soft patches and long paths via a staged content file, Mega Drive button names in Controls. Sega CD is a later milestone.
- **M10 Nintendo 64**: first GPU-rendering core — N64 on Mupen64Plus-Next through an offscreen OpenGL context (backend-agnostic hw-render API for later GL / Vulkan cores); `.z64` / `.v64` / `.n64` hashed as `.z64` (one game, one save folder); analog sticks and C-buttons (keyboard IJKL / TFGH); run-ahead / rewind off for cores whose states are not frame-exact; clear message on PCs without OpenGL 3.3.
- **M11 Sega Dreamcast**: Dreamcast on Flycast through the same OpenGL path; GD-ROM and MIL-CD images (`.gdi`, `.cue`, `.chd`, `.cdi`, `.m3u`) recognised by IP.BIN and matched to the Redump DAT by data-track hash or serial; built-in HLE BIOS or the user's own `dc_boot.bin`; VMU saves per game; analog stick and triggers; run-ahead / rewind off.
- **M12 Sony PSP**: PSP on PPSSPP through the same OpenGL path; UMD images (`.iso`, CISO v1 `.cso`, DVD `.chd`) and `EBOOT.PBP` eboots recognised by content (`PSP GAME` / `PSP_GAME`; PS1 classics stay PlayStation) and matched to the Redump DAT by whole-file hash or serial; the core's system files installed with it; memory stick = the game's save folder; analog stick; run-ahead / rewind off.
- **M13 Nintendo DS**: DS on melonDS DS with the software renderer (no OpenGL needed); `.nds` / `.dsi` / `.ids` recognised by the DS header checksum (DSi-only games reported, encrypted dumps refused at Play with a clear message), matched to the No-Intro (Decrypted) DAT by whole-file hash (streamed above 32 MiB) or the 4-character game code; both screens in one picture (top-bottom, R3 swaps to side by side and resizes the window); touch screen via the mouse or the right stick + R2; L3 = microphone blow; in-game save as `save.srm`; run-ahead / rewind allowed.
- **M14 GameCube + Wii**: GameCube and Wii on Dolphin (OpenGL 3.3, native 1× resolution, single-core CPU); plain and compressed discs (`.iso` / `.gcm` / `.ciso` / `.gcz` / `.wia` / `.rvz` / `.wbfs`), homebrew `.dol` / `.elf` and GameCube `.m3u` recognised by the disc header alone and identified by the 6-character game ID (Redump serial match; no multi-GB hashing; discs, revisions and containers of one game share one save folder); Dolphin's own memory cards / Wii NAND / shader cache per game; save states guarded by length, CRC, exact core version and disc / revision (the launcher hides Continue for states it cannot load); a PC without OpenGL 3.3 gets the graphics message instead of a black screen; GameCube pad with analog triggers and C-stick; Wii Remote + Nunchuk with the pointer aimed by the mouse; teardown watchdog for every core.
- **M7 Handheld Systems**: Game Boy, Game Boy Color and Game Boy Advance on one mGBA core (registry data: three systems, one core, DATs, migration); in-game clocks of Game Boy carts kept across restarts (`rtc.bin`); LCD shader preset; system filter scales to 5+ systems.
- **M6 Multi-system Library**: one library across systems — any folder layout, files classified by extension + content (never folder names); NES on the Mesen core (GPLv3, loaded as a separate library like Snes9x); system registry crate (`system-catalog`) as the only owner of system/core facts; NES No-Intro DAT + metadat (headered and re-headered dumps); cores per system, none required at first run (install prompt on first Play); Library system filter + badges, Controls per system, NES box art; moved/renamed ROMs keep history and saves.
- **M15 RetroAchievements**: rcheevos `rc_client` on the 14 RA systems (all but GameCube / Wii); log in once in launcher Settings (password used once, token stored unencrypted with a visible notice, Log out deletes it); games identified by RA's hash through Nostyra's own disc readers (CHD / CSO / CDI / GDI / cue; PS1 `.pbp` unavailable); softcore by default with hardcore fully enforced but hidden behind one flag until RA approval; leaderboards hardcore-only in rcheevos v12.5.0; in-game overlay (placard, unlock popups with badges + chime, indicators, pause-menu list) with a bundled Unicode font (Latin / Greek / Cyrillic); offline unlocks persisted and sent later; progress in save states and rewind; launcher Achievements tab from the last play.
- **Later**: Big Picture & Deck (spatial-nav UI, `--big-picture` flag, Flatpak, gamescope / Steam Deck validation, packaging `nostyra-runner` into installers; deferred by user 2026-10-18), cloud save sync (WebDAV/Dropbox/GDrive), netplay.

## 7. Non-Functional Requirements

- Input-to-photon latency: ≤ 1 frame over native core cost (run-ahead off); configurable run-ahead 1–2 frames.
- Runner RAM < 150 MB for SNES; launcher idle CPU < 1%.
- Core crash must not crash launcher.
- GPU: Nintendo 64, Dreamcast and PSP need an OpenGL 3.3 driver (other systems need only the wgpu backend); without it the game does not start and the launcher says why (exit 6).
- All network calls optional; app fully usable offline.
- Accessibility: visible focus rings; Big Picture min font 16 px at 1280×800 (deferred, Later).

## 8. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| gamescope focus handoff launcher → runner (deferred with Big Picture & Deck) | Deck UX broken | Prototype when Big Picture & Deck is picked up; fallback: launcher hides itself while runner runs |
| librashader + wgpu integration maturity | Shader feature delay | Plain quad fallback on any chain error (M3); optional device features requested only when offered |
| macOS signing/notarization of downloaded cores | Cores blocked by Gatekeeper | Bundle signed core; document quarantine removal |
| libretro-thumbnails / GitHub index availability or takedown | Missing art | Local artwork cache, offline mode (`NOSTYRA_OFFLINE=1`), placeholder fallback; third-party scrapers are backlog |
| Audio/video sync drift | Crackle/stutter | Dynamic rate control from day one |
| RetroAchievements client approval (hardcore) | Hardcore unlocks not credited | Softcore only; hardcore code + tests kept behind `HARDCORE_UI_ENABLED`; contact RAdmin for user-agent validation after ≥ 6 months public |

## 9. Unresolved Questions

- Optional online provider for descriptions / missing covers (ScreenScraper needs dev credentials) — backlog after M2.
- Distribution channels beyond GitHub Releases + Flathub (winget? Homebrew cask?).
