# Nostyra — Project Overview & PDR

> **Old Worlds. New Ways.**

Last updated: 2026-11-10 · Status: M0–M4 + M6–M20 implemented (Windows; manual checks pending; M16 = web edition Nostyra Online, M17 = Atari 2600 / 7800 / Lynx, M18 = PlayStation 2 on PCEE2, M19 = cloud saves, M20 = online co-op netplay); M5 HD Remaster lives on branch `hd-remaster`; Big Picture & Deck deferred

## 1. Vision

Nostyra is a free, open-source, cross-platform retro game launcher + emulation frontend with a modern, Epic Games Store–style UI. It manages a game library (artwork, metadata, playtime, saves) and runs games through libretro cores. SNES (Snes9x), NES (Mesen), Game Boy / Game Boy Color / Game Boy Advance (mGBA), PlayStation (PCSX ReARMed), Sega Mega Drive / Master System / Game Gear / SG-1000 (Genesis Plus GX), Nintendo 64 (Mupen64Plus-Next, OpenGL), Sega Dreamcast (Flycast, OpenGL), Sony PSP (PPSSPP, OpenGL), Nintendo DS (melonDS DS, two screens, touch via the mouse), Nintendo GameCube / Wii (Dolphin, OpenGL, Wii Remote pointer via the mouse), Atari 2600 (Stella), Atari 7800 (ProSystem), Atari Lynx (Handy) and Sony PlayStation 2 (PCEE2 = PCSX2, OpenGL, the user's own BIOS dump) today; one library for every system, each ROM classified by its content, new systems added as registry data; RetroAchievements (softcore) on 18 of them.

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
- No mobile / console ports. The web edition (M16) is desktop Chrome / Edge only, on thirteen systems, and never replaces the desktop app.
- No monetization, accounts, or telemetry by default.
- No hosted game streaming and no central accounts for netplay: online co-op exchanges inputs peer to peer; the only server is a small self-hosted signaling + relay service that stores nothing after a room closes.

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
| Emulation | libretro API host; Snes9x (SNES), Mesen (NES), mGBA (GB, GBC, GBA), PCSX ReARMed (PS1), Genesis Plus GX (MD, SMS, GG, SG-1000), Mupen64Plus-Next (N64), Flycast (Dreamcast), PPSSPP (PSP), melonDS DS (NDS), Dolphin (GameCube, Wii), Stella (Atari 2600), ProSystem (Atari 7800), Handy (Atari Lynx), PCEE2 (PlayStation 2) | Swappable cores per system (registry), multi-system path |
| Process model | Launcher and runner are separate processes | Crash isolation, latency, CLI launch |
| Video | winit + wgpu + librashader | Vulkan/Metal/DX12, RetroArch slang shaders |
| GPU cores | Backend-agnostic hw-render API in `libretro-host`; OpenGL backend `hw-render-gl` (hidden SDL3 context, FBO readback into the wgpu presenter) | Shaders / overlay / thumbnails unchanged; Vulkan and zero-copy plug in later |
| Audio/Input | SDL3 | Unified gamepad DB, Steam Input friendly |
| Storage | SQLite (rusqlite, WAL) | Embedded, fast, zero setup |
| Web edition (M16) | Same React launcher built with `vite --mode web` over a TS backend (`src/backend/web/`); shared Rust rules compiled to wasm (`crates/web-bindings`); own sqlite-wasm DB in OPFS; EmulatorJS 4.2.3 self-hosted, one iframe per game; static site at nostyra.com/play (Vercel, manual deploy) | No server, no upload; saves / config / BIOS shared with the desktop as files in one Data folder, never the desktop DB |
| Achievements | rcheevos v12.5.0 (MIT) vendored unmodified, `rc_client` in the runner behind a safe single-thread wrapper; softcore default, hardcore coded but hidden until RA approves the client | Same library and data as RetroArch; RA requires ≥ 6 months public before hardcore credit |
| Netplay (M20) | Rollback with ggrs 0.13 over WebRTC data channels (matchbox 0.15, webrtc-rs); self-hosted signaling server (`apps/netplay-server`, axum) + coturn behind the VPS's nginx; up to 4 players + spectators; only inputs travel, host sends SRAM / state at start | GGPO-style rollback feels local at typical pings; WebRTC punches NAT and relays through TURN without port forwarding; the same stack can later run in the browser (M20b) |

### Snes9x license note
Snes9x has a non-commercial, non-GPL license. Nostyra (GPLv3) does **not** link Snes9x source; it loads the `snes9x_libretro` core as a separate dynamic library at runtime (same model as RetroArch). Nostyra stays free; no commercial use of Snes9x. The NES core `mesen_libretro` (Mesen 0.9.9) is GPLv3, downloaded and loaded the same way. Mesen2 was the first choice but its buildbot build rejected every ROM (M6 spike); switching later is a registry entry. The handheld core `mgba_libretro` (MPL-2.0, file-level copyleft, GPL-compatible) is downloaded and loaded the same way; no BIOS is required or shipped. The PlayStation core `pcsx_rearmed_libretro` (GPL-2.0-or-later) is downloaded and loaded the same way. The Sega core `genesis_plus_gx_libretro` has a non-commercial licence like Snes9x: downloaded at runtime from the buildbot, never bundled or shipped in a paid distribution. The Nintendo 64 core `mupen64plus_next_libretro` and the Dreamcast core `flycast_libretro` and the PSP core `ppsspp_libretro` (all GPL-2.0-or-later) are downloaded and loaded the same way; PPSSPP's system files (`PPSSPP.zip`: fonts, dialog atlas, compat table) are downloaded from the buildbot with it, never bundled. The Nintendo DS core `melondsds_libretro` (melonDS DS, GPL-3.0-or-later) is downloaded and loaded the same way; it runs on its built-in FreeBIOS and generated firmware, so only decrypted dumps play. The GameCube / Wii core `dolphin_libretro` (Dolphin, GPL-2.0-or-later) is downloaded and loaded the same way, with its system files (`Dolphin.zip`) from the buildbot, never bundled; it boots with its built-in IPL replacement and Wii keys. The Atari cores `stella_libretro` (GPL-2.0-or-later), `prosystem_libretro` (GPL-2.0) and `handy_libretro` (zlib) are downloaded and loaded the same way and need no BIOS. The PlayStation 2 core `pcee2_libretro` (PCEE2, upstream PCSX2 v2.9, GPL-3.0-or-later) is downloaded and loaded the same way; it has **no BIOS replacement**, so a PS2 game starts only with a BIOS the user dumped from their own console. **BIOS policy**: Nostyra never bundles, downloads or links BIOS files; PS1, Dreamcast, DS, GameCube and Wii games run on the core's built-in BIOS replacement, and a BIOS the user dumped from their own console can be added from Settings → BIOS (a Dreamcast BIOS only with a known checksum; a PS2 BIOS is validated by its ROMDIR version entry, not by a checksum list, and the game refuses to start without one).

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
- **M16 Web edition (Nostyra Online)**: `https://nostyra.com/play` for desktop Chrome / Edge (other browsers → gate page): pick a ROM folder (File System Access), same Home / Library / Detail / Settings, play NES, SNES, GB, GBC, GBA, MD, SMS, GG, SG-1000 and PS1 via EmulatorJS 4.2.3; content ids identical to the desktop (Rust identity rules in wasm), so a Data folder shared with the desktop (browser storage by default; desktop default moved to `Documents\Nostyra` because Chrome blocks AppData) gives the same in-game saves, TOML settings, controls and BIOS; web save states kept apart (`web-slot*` / `web-auto`); web play time imported by the desktop by content sha1; single tab; licences page; desktop-only features shown disabled. N64 / NDS / PSP and multi-disc switching → M16b.
- **M17 Atari 2600 / 7800 / Lynx**: three cartridge systems on three small cores (Stella, ProSystem, Handy; never bundled, no BIOS); `.a26`, headered `.a78` / `.lnx`, raw `.lyx`, BS93 `.o` / `.bll` recognised per extension (a C object `.o` ignored silently); a loose `.bin` (shared with Mega Drive / PS1) listed as 2600 / 7800 only when the No-Intro DAT knows its hashes, otherwise a problem row with a rename hint; headered dumps match headerless DAT entries; console switches on the pad (2600 Select / Reset / difficulty / colour, 7800 Reset / Pause / hold-difficulty, Lynx Option 1 / 2); Lynx EEPROM as `content.eeprom` in the game's save folder, rotation automatic; states with run-ahead and rewind; RetroAchievements on all three; zipped `.a26` / `.a78` / `.lnx` / `.lyx` / `.o`; Nostyra Online plays all three (EmulatorJS stella2014, prosystem, handy).
- **M18 PlayStation 2**: PS2 on PCEE2 (the PCSX2 libretro core; OpenGL on the existing 3.3 context — the core adds its own shared GS context — with the core's own Vulkan readback as the no-GL fallback, native 1×); `.iso`, `.bin` / `.cue`, CD / DVD `.chd`, `.cso` / LZ4 `.zso`, `.m3u` up to 9 GiB recognised by content (`PLAYSTATION` volume + `SYSTEM.CNF` `BOOT2`); identity by serial (no 4–8 GB hash; volume id + boot path for serial-less discs); the first system that **requires the user's own BIOS dump**: any 4–8 MiB ROM with a ROMDIR `ROMVER` is accepted under any name (`.BIN` / `.ROM0`), decoded to "v1.60 · USA · 2002-02-07" labels, stored with its `.rom1` / `.nvm` / `.mec` sidecars in `system/pcsx2/bios/`, picked per disc region (then newest) by one rule shared by Settings and the runner, and a launch with no BIOS is refused before the core loads; shared memory cards `system/pcsx2/memcards/Mcd001.ps2` / `Mcd002.ps2` as on a real console; 96 MiB states with strict core-version and disc / revision guards, run-ahead / rewind off; DualShock 2 with both sticks and L3 / R3; RetroAchievements (console 21, hardcore rules from the registry); desktop only (no web core); Linux / macOS builds untested.
- **M20 Online co-op (netplay)**: desktop launcher Detail → Play online (Host game / Join with code / Watch with code); 6-character room codes; rollback netplay for up to 4 players (multitap per core from the registry) plus up to 8 spectators; compatibility fingerprint refuses mismatched dumps / cores / options / region with the exact field; host's SRAM / RTC / state synced at start; rewind / fast-forward / run-ahead / load state / reset / disc swap off, non-pausing menu, host-only saves, softcore achievements, desync warning + host Resync; HUD (ping, rollbacks, RELAY); Settings → Netplay (nickname, input delay, HUD, Prefer relay best effort, server address). Systems: SNES, NES, MD / SMS / GG / SG-1000, Atari 2600, GB / GBC / GBA (shared screen) = Yes; PS1, Atari 7800 = Experimental; Lynx, NDS, GL cores = No. Web peers are M20b.
- **Later**: Big Picture & Deck (spatial-nav UI, `--big-picture` flag, Flatpak, gamescope / Steam Deck validation, packaging `nostyra-runner` into installers; deferred by user 2026-10-18), web peers for netplay (M20b), relay-only netplay (patched matchbox), netplay reconnect / mid-session join.

## 7. Non-Functional Requirements

- Input-to-photon latency: ≤ 1 frame over native core cost (run-ahead off); configurable run-ahead 1–2 frames.
- Runner RAM < 150 MB for SNES; launcher idle CPU < 1%.
- Core crash must not crash launcher.
- GPU: Nintendo 64, Dreamcast, PSP, GameCube and Wii need an OpenGL 3.3 driver (other systems need only the wgpu backend); without it the game does not start and the launcher says why (exit 6). PlayStation 2 prefers OpenGL 3.3 and otherwise runs on the core's own Vulkan readback device (slower, one more frame of latency); full speed needs a ~4-core Haswell-class CPU.
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
| Netplay exposes IP addresses (full mesh; Prefer relay is best effort) | Privacy | README Privacy says so plainly, no IP-hiding claim; logs never hold codes / tokens / IPs; relay-only via a matchbox patch is backlog |
| Netplay desync on a core / game (RTC, Experimental cores) | Session drifts apart | RAM + SRAM checksum every interval → HUD warning → host Resync; Experimental badge; per-core registry flag turns a core off |

## 9. Unresolved Questions

- Optional online provider for descriptions / missing covers (ScreenScraper needs dev credentials) — backlog after M2.
- Distribution channels beyond GitHub Releases + Flathub (winget? Homebrew cask?).
