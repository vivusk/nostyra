# Nostyra

> **Old Worlds. New Ways.**

Free, open-source, cross-platform retro game launcher and libretro frontend.
Plays SNES (Snes9x core), NES (Mesen core), Game Boy, Game Boy Color and Game Boy Advance (one mGBA core),
PlayStation (PCSX ReARMed core), Sega Mega Drive / Genesis, Master System, Game Gear and SG-1000 (one Genesis
Plus GX core), Nintendo 64 (Mupen64Plus-Next core, OpenGL), Sega Dreamcast (Flycast core, OpenGL), Sony
PSP (PPSSPP core, OpenGL), Nintendo DS (melonDS DS core) and Nintendo GameCube / Wii (Dolphin core, OpenGL);
more systems are registry entries. Status: **M15 — RetroAchievements** (log in once in Settings; softcore
achievements on 14 systems with an in-game overlay, unlock chime, pause-menu list and a launcher Achievements tab;
offline play of games played online once, unlocks sent later; see [RetroAchievements](#retroachievements)) implemented on Windows on top of M14
GameCube + Wii (plain and compressed discs identified by their game ID without hashing gigabytes; Wii Remote +
Nunchuk with the pointer aimed by the mouse), M13 Nintendo DS, M12 Sony PSP, M11 Sega Dreamcast, M10 Nintendo 64, M9 Sega 8/16-bit, M8 PlayStation, M7 handheld systems, M6 multi-system library (one ROM folder organised any way you like, every file classified
by its content, moved or renamed ROMs keep history and saves), M4 controls & resume and M3 play features; manual
checks and macOS/Linux pending.

See [docs/project-overview-pdr.md](docs/project-overview-pdr.md) and
[docs/system-architecture.md](docs/system-architecture.md). First public release: **v0.1.0** (Windows x64
portable zip), see the [release notes](docs/release-notes-v0.1.0.md) and
[Install (Windows portable)](#install-windows-portable).

## Install (Windows portable)

1. Download `Nostyra-<version>-windows-x64-portable.zip` from the **official GitHub Releases page only**
   (never from mirrors or re-uploads).
2. Optional: right-click the zip → **Properties** → tick **Unblock** → OK, before extracting. This clears the
   "downloaded from the internet" mark, so the extracted files do not inherit it and the warning below
   usually does not appear.
3. Extract the zip to a folder you can write to (not `C:\Program Files`) and run `Nostyra.exe`.
4. **"Windows protected your PC"**: Nostyra's exe is not code-signed yet, so Microsoft Defender SmartScreen
   may block the first start of a new download ("Unknown publisher"). Click **More info**,
   then click **Run anyway**.

Needs Windows 10 / 11 x64 and the Microsoft Edge WebView2 Runtime (built into Windows 11 and updated
Windows 10). See the [release notes](docs/release-notes-v0.1.0.md) for requirements, known limits and how
to report bugs.

## Build

Requirements: Rust (version pinned in `rust-toolchain.toml`, installed
automatically by rustup), CMake on PATH (SDL3 is built from source; on Windows the
cmake bundled with Visual Studio works via the `CMAKE` env var), Python 3 (only to
rebuild the test ROM).
Linux also needs: `libudev-dev libasound2-dev libpulse-dev libpipewire-0.3-dev
libxkbcommon-dev libwayland-dev` and X11 dev packages.

```sh
# Fetch the libretro cores into ./cores/ (snes9x = SNES, mesen = NES, mgba = GB/GBC/GBA,
# pcsx_rearmed = PlayStation, genesis_plus_gx = Mega Drive/Master System/Game Gear/SG-1000,
# mupen64plus_next = Nintendo 64, flycast = Dreamcast, ppsspp = PSP + its system files in cores/PPSSPP/,
# melondsds = Nintendo DS, dolphin = GameCube + Wii + its system files in cores/dolphin-emu/)
./scripts/fetch-snes9x-core.sh          # Linux / macOS / Git Bash
./scripts/fetch-mesen-core.sh
./scripts/fetch-mgba-core.sh
./scripts/fetch-pcsx-rearmed-core.sh
./scripts/fetch-genesis-plus-gx-core.sh
./scripts/fetch-mupen64plus-next-core.sh
./scripts/fetch-flycast-core.sh
./scripts/fetch-ppsspp-core.sh
./scripts/fetch-melondsds-core.sh
./scripts/fetch-dolphin-core.sh
.\scripts\fetch-snes9x-core.ps1         # Windows PowerShell
.\scripts\fetch-mesen-core.ps1
.\scripts\fetch-mgba-core.ps1
.\scripts\fetch-pcsx-rearmed-core.ps1
.\scripts\fetch-genesis-plus-gx-core.ps1
.\scripts\fetch-mupen64plus-next-core.ps1
.\scripts\fetch-flycast-core.ps1
.\scripts\fetch-ppsspp-core.ps1
.\scripts\fetch-melondsds-core.ps1
.\scripts\fetch-dolphin-core.ps1

cargo build --workspace
cargo test --workspace
cargo test -p artwork-cache -- --ignored   # optional: one real thumbnail download
```

### Portable build (Windows, no installer)

```powershell
.\scripts\build-portable.ps1               # -SkipBuild reuses target\release\*.exe
```

Writes `dist\Nostyra-<version>-windows-x64-portable.zip`: `Nostyra.exe`,
`nostyra-runner.exe`, the Visual C++ runtime DLLs the runner needs (copied app-local
from `System32`), `libretro-database\`, `shaders\`, `licenses\` (Nostyra's GPL-3.0, the rcheevos MIT
licence, the Noto Sans OFL-1.1 text, the libretro-database `LICENSE` / `SOURCE.txt` and slang-shaders
`SOURCE.txt`, plus generated notices: `THIRD-PARTY-RUST.txt` from [cargo-about](https://github.com/EmbarkStudios/cargo-about)
0.9.2 with `about.toml` / `about.hbs`, `THIRD-PARTY-NATIVE.txt` from `scripts/generate-native-notices.ps1` (SDL3,
glslang, SPIRV-Cross, SQLite, VC++ runtime) and `THIRD-PARTY-NPM.txt` from `scripts/generate-npm-notices.mjs`),
`RELEASE-NOTES.md` (copied from `docs/release-notes-v<version>.md`, which must exist) and `portable.txt`.
Needs `cargo install cargo-about --version 0.9.2 --locked --features cli`, Node.js and `npm ci` in
`apps/launcher`. Users unzip
and run `Nostyra.exe`. The `portable.txt` marker keeps everything in the folder: data in
`data\` beside the exe (Settings → Data folder can still move it; that choice is stored
beside the exe, never in `%APPDATA%`). Cores download on first Play as usual. Needs the
Microsoft Edge WebView2 Runtime (built into Windows 11 and updated Windows 10); without
it the launcher shows a message box with the download link instead of closing silently.

### Launcher (Tauri 2 + React)

Also requires Node.js LTS (npm). Linux additionally needs `libwebkit2gtk-4.1-dev
librsvg2-dev libxdo-dev libssl-dev`.

```sh
cd apps/launcher
npm ci && npm run build   # once, and after UI changes, before cargo builds the launcher
npm run tauri dev         # dev window with hot reload (also builds nostyra-runner)
npm run typecheck
```

Workspace cargo commands (`cargo clippy/test --workspace`) work without npm: the
launcher's `build.rs` creates an empty `apps/launcher/dist/` when missing.
The launcher looks for `nostyra-runner` next to its own executable; override with
the `NOSTYRA_RUNNER` env var.

Dev only: in a debug build, run `window.__TAURI_INTERNALS__.invoke("seed_fake_library", { count: 1000 })`
in the devtools console to insert fake games (capped at 5 000, re-runnable). Remove the
`seed-library` folder in Settings to clean up. Release builds return an error.

## Layout

| Path | Purpose |
|------|---------|
| `apps/runner` | `nostyra-runner` binary (plays one ROM with one core) |
| `apps/launcher` | `nostyra-launcher` desktop app: React UI (`src/`) + Tauri backend (`src-tauri/`) |
| `crates/libretro-host` | core loading + hand-written libretro FFI |
| `crates/video-renderer` | wgpu frame presentation |
| `crates/hw-render-gl` | OpenGL backend for cores that render on the GPU: hidden SDL3 GL context, framebuffer, readback |
| `crates/audio-output` | SDL3 audio with dynamic rate control |
| `crates/input-mapping` | SDL3 gamepad/keyboard → libretro joypad + analog sticks, pointer state (DS touch screen, Wii Remote pointer) |
| `crates/storage-db` | launcher SQLite database: schema migrations + repositories |
| `crates/game-library` | ROM folder scan, header-aware CRC32/SHA1 (streamed for large files), No-Intro / Redump DAT matching (hash, disc serial, DS game code, GameCube / Wii game ID), multi-file disc games |
| `crates/disc-image` | disc images without loading them: cue/gdi/m3u sheets, sectors (raw, ISO, CHD incl. GD-ROM and DVD, DiscJuggler CDI, CISO `.cso`), ISO9660 paths, PlayStation disc id + serial, Dreamcast IP.BIN, PSP disc id (`PARAM.SFO`, `UMD_DATA.BIN`), PBP kind (PS1 / PSP), GameCube / Wii disc headers (ISO, CISO, GCZ, WIA, RVZ, WBFS) and `.dol` / `.elf` programs, serial rules per system |
| `crates/rom-patch` | soft-patching: IPS / UPS / BPS applied to the ROM in memory |
| `crates/http-download-util` | shared HTTPS client (timeouts, offline switch), size-capped downloads |
| `crates/config-layers` | gameplay settings: TOML layers (global → system → core → game), merge, validated writes |
| `crates/artwork-cache` | libretro-thumbnails lookup, cover/hero download, re-encode, atomic cache |
| `crates/achievements` | RetroAchievements: vendored rcheevos behind a safe single-thread `rc_client` wrapper, HTTP transports (real + fake), login, credentials, offline unlock journal; used by the runner (session, overlay) and the launcher (Settings login, Achievements tab, startup journal flush) |
| `test-assets/achievements` | fake RetroAchievements sets for the test ROMs (one achievement + one leaderboard on each ROM's RAM frame counter), generated by `generate-fake-sets.py`; used by tests through `NOSTYRA_RA_FAKE_SERVER`, never by users |
| `test-assets/roms` | self-written GPLv3 test ROMs (`nostyra-test.sfc`, `.nes`, `.gb`, `.gbc`, `.gba`, `.gen`, `.sms`, `.gg`, `.sg`, `.z64`), PlayStation test discs (`nostyra-test-psx*.cue/.bin/.m3u`), the Dreamcast test program and discs (`nostyra-test-dc.elf`, `nostyra-test-dc*.gdi/.cue/.m3u`), the PSP test program and containers (`nostyra-test-psp.elf/.iso/.cso`, `nostyra-test-psp/EBOOT.PBP`) the DS test ROMs (`nostyra-test.nds` homebrew, `nostyra-test-save.nds` retail-like with an EEPROM save) and the GameCube / Wii test programs (`nostyra-test.dol`, `nostyra-test-wii.dol`) |
| `third-party/libretro` | vendored `libretro.h` (MIT) |
| `third-party/rcheevos` | vendored [rcheevos](https://github.com/RetroAchievements/rcheevos) v12.5.0 (MIT), `include/` + `src/` unmodified, compiled by `crates/achievements/build.rs`; see its `SOURCE.txt` |
| `third-party/libretro-database` | bundled No-Intro DATs (SNES, NES, GB, GBC, GBA, N64, NDS, Mega Drive, Master System, Game Gear, SG-1000), the Redump PlayStation, Dreamcast, PSP, GameCube and Wii DATs + `metadat/` attribute DATs (CC-BY-SA-4.0), refreshed by `scripts/update-libretro-dat.*` |
| `assets/fonts` | Noto Sans Regular + Bold v2.015 (unhinted TTF, SIL OFL-1.1, `OFL.txt` alongside; source and hashes in `SOURCE.txt`), embedded in the runner for the achievement overlay text, rasterized with [fontdue](https://github.com/mooman219/fontdue) 0.9.4 (MIT OR Apache-2.0 OR Zlib) |
| `third-party/slang-shaders` | bundled CRT and LCD shader presets (crt-easymode, crt-geom, crt-lottes, zfast-lcd) from libretro/slang-shaders, unmodified; see its `SOURCE.txt` |

## Legal

Nostyra does not ship or download games or BIOS files. Use games you own and a BIOS
dumped from your own console.
Game identification uses the No-Intro SNES, NES, Game Boy, Game Boy Color, Game Boy
Advance, Nintendo 64, Nintendo DS (Decrypted), Sega Mega Drive - Genesis, Master System - Mark III, Game Gear and SG-1000 DATs and
the Redump PlayStation, Dreamcast, PlayStation Portable, GameCube and Wii DATs as compiled by
[libretro-database](https://github.com/libretro/libretro-database) (CC-BY-SA-4.0),
bundled unmodified in `third-party/libretro-database/`; developer, publisher, genre,
year and player counts come from the same project's `metadat` files (CC-BY-SA-4.0; the
PlayStation file has its long `description` lines removed).
Emulator cores are never bundled: each one is downloaded from the libretro buildbot on
first use. Snes9x and Genesis Plus GX carry **non-commercial** licences, so Nostyra must
not be sold with them; Mupen64Plus-Next (Nintendo 64), Flycast (Dreamcast) and PPSSPP (PSP) are
GPL-2.0-or-later; melonDS DS (Nintendo DS) is GPL-3.0-or-later; Dolphin (GameCube, Wii) is GPL-2.0-or-later.
PPSSPP's system files (fonts, dialog images, compatibility table) and Dolphin's (`Dolphin.zip`) come from the
same buildbot with the core and are never bundled either.
Cover and screenshot art comes from
[libretro-thumbnails](https://github.com/libretro-thumbnails/libretro-thumbnails): it is
never bundled, only downloaded per game at runtime into a local cache you can clear in
Settings (attribution shown there too).
CRT shader presets `crt-easymode` (GPL), `crt-geom` (GPL-2.0-or-later) and `crt-lottes`
(public domain) and the LCD preset `zfast-lcd` (GPL-2.0-or-later) come unmodified from
[libretro/slang-shaders](https://github.com/libretro/slang-shaders) with their licence
headers, in `third-party/slang-shaders/` (commit and per-file licences in `SOURCE.txt`).
They run through [librashader](https://github.com/SnowflakePowered/librashader)
(MPL-2.0 OR GPL-3.0-only; used under MPL-2.0). In the launcher bundle they ship as
`shaders/` in the app's resource folder.
RetroAchievements support is built on
[rcheevos](https://github.com/RetroAchievements/rcheevos) v12.5.0 (MIT), vendored unmodified in
`third-party/rcheevos/` with its `LICENSE` (tag and commit in `SOURCE.txt`).
The achievement overlay draws text with the Noto Sans fonts (SIL Open Font License 1.1,
Copyright 2022 The Noto Project Authors) bundled unmodified in `assets/fonts/` with
`OFL.txt`, rasterized by fontdue (MIT OR Apache-2.0 OR Zlib), with titles normalised by
unicode-normalization (MIT OR Apache-2.0); the unlock chime is synthesized in code (no audio
asset). Achievement data, badges and game icons come from
[RetroAchievements](https://retroachievements.org) at runtime and are never bundled.

Network use is optional: scanning, metadata, Home, Detail and Play all work offline; the
artwork fetch stops quietly after a few failed games and retries later. Set
`NOSTYRA_OFFLINE=1` to disable all launcher downloads.
Licensed under GPL-3.0-or-later — see [LICENSE](LICENSE).

## Privacy

RetroAchievements is optional and off until you log in (Settings → RetroAchievements).
Your password is sent once to retroachievements.org to get a login token and is never
stored. The username and token are saved **unencrypted** in
`<data folder>/achievements/credentials.json`: anyone who can read files of your PC account
can read the token (it is not your password and can be revoked by changing your RA
password). Log out deletes the file. The launcher hands the token to the game runner through
environment variables (not the command line). While logged in, the runner sends your
unlocks, leaderboard entries and rich presence to RetroAchievements; unlocks earned offline
wait in `<data folder>/achievements/pending/` and are sent the next time you play or start the
launcher. Games played online keep the last server answers (login, achievement set, unlock list) in
`<data folder>/achievements/offline-cache/<user>/` so they work offline; the cached login answer holds the
token, so the folder is just as sensitive as `credentials.json`. It is deleted on Log out, when RA rejects
the token, and when you log in as another account. To find the game's achievement set the runner sends the game's RetroAchievements hash
(an MD5 of the game data, never the file itself) and a user agent naming Nostyra, the core and
rcheevos versions. Per-game achievement lists and badges are cached in `<data folder>/achievements/games/`
and `<data folder>/cache/achievement-badges/`. Nothing else is sent.

Logs stay on your PC: `<data folder>/logs/` holds the launcher and game logs. They contain file paths
(ROM, core and data folder paths, which may include your Windows user name) but never your RetroAchievements
password or token. Nothing is uploaded automatically: a bug report is a zip you create and attach yourself
(see [Logs and bug reports](#logs-and-bug-reports)).

## Logs and bug reports

The launcher writes `<data folder>/logs/launcher-YYYYMMDD-HHMMSS.log` each time it starts, and every game
session writes `runner-YYYYMMDD-HHMMSS-<system>.log` (the game runner also keeps printing to stderr; the
error toast's **Details** shows its last lines). Timestamps are local time. Each file stops at 10 MiB with a
`log truncated (size limit)` line. Old files are deleted automatically at every launcher and game start: the
newest 10 launcher logs and 30 game logs are kept, any log older than 14 days and any bug-report zip older
than 7 days is removed. `NOSTYRA_LOG` (EnvFilter syntax, e.g. `set NOSTYRA_LOG=debug`) changes the level for
both, then `RUST_LOG`; the default is `info`. Changing the data folder starts a fresh `logs/` in the new
folder (old logs are not moved).

To report a bug: **Settings → About → Logs → Create bug report** (or **Create bug report** on a game's error
toast). It writes `<data folder>/logs/nostyra-bug-report-YYYYMMDD-HHMMSS.zip` with the current and previous
launcher logs, the 5 newest game logs and `system-info.txt` (Nostyra version, Windows version, data-folder
kind portable / default / custom, installed cores and versions), opens the folder with the zip selected and
shows "Bug report saved — attach it to your GitHub issue". Any `token` / `password` / `t=` value is scrubbed
from the copied logs as a safety net. **Open logs folder** opens `logs/` itself.

## Your ROM folder

Add one folder (or several) in the launcher. Organise it any way you like: `ROM/SNES`,
`ROM/NES`, `ROM/Favorites`, everything mixed — folder names are never used. Each file is
classified by its extension and content: `.sfc` / `.smc` = Super Nintendo, `.nes` with an
iNES header = NES (a `.nes` without a header is listed as not scanned), `.gb` = Game Boy and
`.gbc` = Game Boy Color (both need the cartridge header logo bytes), `.gba` = Game Boy
Advance (needs the ARM branch at its start). A `.gb` renamed to `.gbc` (or back) still plays
but is not identified (the No-Intro DATs name them by extension). No BIOS is needed; mGBA
uses `gb_bios.bin` / `gbc_bios.bin` / `gba_bios.bin` from `system/` if you add them. PlayStation
discs are recognised by their content too (see [PlayStation](#playstation)), and so are Sega
files (see [Sega](#sega-mega-drive-master-system-game-gear-sg-1000)), Nintendo 64 files (see
[Nintendo 64](#nintendo-64)), Dreamcast discs (see [Sega Dreamcast](#sega-dreamcast)), PSP games (see
[Sony PSP](#sony-psp)), DS games (see [Nintendo DS](#nintendo-ds)) and GameCube / Wii discs (see
[Nintendo GameCube and Wii](#nintendo-gamecube-and-wii)). Moving a ROM to
another folder or renaming it keeps its play time, favorite, edited metadata, saves and
per-game settings. No emulator core is needed up front: the first Play of a system offers
to download its core (also in Settings → Emulator cores).

## Run

```sh
cargo run -p nostyra-runner -- --core cores/snes9x_libretro.dll --rom path/to/game.sfc
cargo run -p nostyra-runner -- --core cores/mesen_libretro.dll --rom path/to/game.nes --system nes
cargo run -p nostyra-runner -- --core cores/mgba_libretro.dll --rom path/to/game.gba --system gba
cargo run -p nostyra-runner -- --core cores/pcsx_rearmed_libretro.dll --rom path/to/game.cue --system psx
cargo run -p nostyra-runner -- --core cores/mupen64plus_next_libretro.dll --rom path/to/game.z64 --system n64
cargo run -p nostyra-runner -- --core cores/flycast_libretro.dll --rom path/to/game.gdi --system dreamcast
cargo run -p nostyra-runner -- --core cores/ppsspp_libretro.dll --rom path/to/game.iso --system psp
cargo run -p nostyra-runner -- --core cores/melondsds_libretro.dll --rom path/to/game.nds --system nds
cargo run -p nostyra-runner -- --core cores/dolphin_libretro.dll --rom path/to/game.rvz --system gc
```

Default keyboard: arrows, Z = B, X = A, A = Y, S = X, Q/W = L/R, E/R = L2/R2, Enter = Start,
Right Shift = Select; on systems with analog sticks (Nintendo 64, Dreamcast, PSP, GameCube, Wii) I/J/K/L = left stick and
T/F/G/H = right stick (on the DS T/F/G/H move the touch cursor). Gamepads work out of the box (SNES layout by position).
Default hotkeys: F1 save state, F2 / F3 previous / next slot (0-9), F4 load state,
F11 fullscreen, F10 pause menu, P turbo A on / off (every system: player 1's A
button presses itself 3 frames on / 3 off, about 10 times a second, until P again;
"Turbo A: On/Off" on screen; off at each start). Buttons and hotkeys are rebindable (see
[Controls](#controls-and-hotkeys)); Esc, pad Guide and Select+Start held 0.5 s always
open the pause menu:
resume, save / load state with a thumbnail grid, reset game (soft reset, asks
"Reset game?", No by default; in-game saves kept), options (shader, scaling,
run-ahead, rewind; "Save for this game" / "Save as default" write the changed keys
to the game / global config layer, "Reset game settings" removes the game's keys), quit (asks "Quit game?", No by default).
Arrows / D-pad move, Enter / A confirm, Esc / Backspace / B back. The game is paused
while the menu is open (play time keeps counting). Closing the window (Alt+F4) quits
at once, SRAM saved (and a resume point, see [Resume](#resume)). Hold `` ` `` (key left of 1) or R2 on a gamepad to
fast-forward (uncapped speed, audio muted). With `rewind.enabled = true` in a
config layer (off by default), hold Backspace or L2 to rewind (2× speed, audio
muted; `rewind.buffer_mb`, default 64, caps the history). The window title shows the current slot
and result, and F1–F4 also
show a short status ("Slot 3 · Saved") in the bottom-left of the picture for 3 s.
Options: `--fullscreen`, `--audio-sync` (force audio-paced timing; auto-selected on
non-60 Hz displays), `--state-slot N` (starting slot, 0-9), `--load-state N` (start
from slot N and make it current; an empty or incompatible slot shows an error and the
game starts normally; not combinable with `--state-slot`), `--load-state auto` (start
from the resume point; "Resumed", or "Could not resume" and a normal start),
`--data-dir DIR`.
Saves live in the per-user data dir (`%APPDATA%\Nostyra\data` on Windows by default; Settings → Data folder → Change… picks another folder, optionally moving the data there, and restarts):
one folder per game, `saves/<system>/<ROM name> [<ID>]/` (`snes`, `nes`, `gb`, `gbc`, `gba`, `n64`, `genesis`,
`sms`, `gg`, `sg1000`, `psx`, `dreamcast`, `psp`, `nds`, `gc`, `wii`),
holding `save.srm`, `rtc.bin` (cartridge clock of Game Boy games with one, e.g. Pokémon
Gold, so the in-game time keeps running while the game is closed) and
`slotN.state` (+ `.json`, and a `slotN.png` thumbnail of the game picture, without
on-screen text or shader), plus `auto.state` / `.json` / `.png` for the resume point;
`system/` for BIOS files; `temp/staged-content/` holds a short-lived copy of a patched or long-path
ROM for cores that only open files by path (removed when the game closes). A `save.srm` shorter than the core's save memory loads as is (mGBA
reports 128 KiB for a GBA game until it first touches its save chip); one longer than it is
kept as `save.srm.bak` before the next save trims it. `<ID>` is the first 8 hex digits of the
game's SHA-1 (e.g. `Chrono Trigger (USA) [6B47BB75]`): the folder is found by that id, so renaming
or moving the ROM keeps its saves, and you may rename the name part yourself (keep ` [<ID>]`). A
soft-patched game shares the original's folder; all discs of a PS1 `.m3u` game share one. Folders
from older versions (`<ROM name>` only) get their id the next time the launcher starts.
Soft-patching: put patches with the ROM's file name next to it (`Game.sfc` +
`Game.bps` / `.ups` / `.ips`) and they are applied in memory at start, the ROM file
is never modified. Several patches chain in this order: `.bps`, `.bps1`, `.bps2`, …,
then `.ups`, `.ups1`, …, then `.ips`, `.ips1`, … (numbering stops at the first gap);
each patch must be made for the output of the one before. Saves are shared with the
unpatched game (same ROM name). If any patch in the chain fails (e.g. made for a
different ROM) the game starts unpatched; the title shows `[IPS patch]`,
`[3 patches]` or `[patch failed]`.
CRT / LCD shaders: `video.shader` in the config files (`none` by default, or `crt-easymode`,
`crt-geom`, `crt-lottes`, `zfast-lcd` for handhelds, or an absolute `.slangp` path). The preset builds in the
background (plain picture meanwhile); a bad preset keeps the plain picture and shows
"Shader failed". Bundled presets are looked up in `--shaders-dir DIR` (the launcher passes
its copy), then `NOSTYRA_SHADERS`, then `shaders/` next to the runner, then (debug builds)
`third-party/slang-shaders`. A standalone release runner needs `NOSTYRA_SHADERS` set to
that folder (or a copy at `shaders/` beside the exe). librashader's on-disk shader
cache only works on Vulkan (`PIPELINE_CACHE`), so on DX12/Metal each preset is rebuilt
at start (≈ 0.3–1 s in release, picture kept meanwhile).
Run-ahead (`latency.run_ahead_frames` = 1 or 2) hides 1–2 frames of the game's own
input lag by emulating ahead and rolling back each frame (single instance); it is off
while fast-forwarding, rewinding or in the menu, and refused (with rewind) for cores whose save
states are incomplete or not frame-exact (Nintendo 64, Dreamcast, PSP, GameCube, Wii); the pause menu shows the reason. On the
Nintendo DS run-ahead is allowed but too heavy for most PCs (one run-ahead frame costs about 18 ms, more than a
whole 60 Hz frame), so expect dropped frames; rewind works normally.
`--print-config` prints the effective settings as JSON (after the core and ROM load;
the core may print its own text after it) and exits.

Launcher mode: `--ipc` (JSON events on stdout + a pending play-session file under
`<data>/sessions/pending/`); `--probe-core <lib>` checks a core and prints its identity.
Exit codes: 0 ok, 2 bad arguments, 3 core failed to load, 4 ROM failed to load (the error line carries the
core's own reason when it gives one, e.g. "ROM isn't valid"; the launcher says "This game file could not be loaded"),
5 runtime error, 6 the core needs graphics this PC cannot provide (OpenGL 3.3).

### Gameplay settings (config files)

Plain TOML files under `<data dir>/config/` (`%APPDATA%\Nostyra\data\config` on Windows),
lowest precedence first: `global.toml`, `system/<system>.toml` (`snes`, `nes`, `gb`, `gbc`, `gba`, `n64`,
`genesis`, `sms`, `gg`, `sg1000`, `psx`, `dreamcast`, `psp`, `nds`, `gc`, `wii`), `core/<core>.toml` (`snes9x`, `mesen`, `mgba` — shared by `gb`, `gbc`
and `gba`, `mupen64plus-next`, `genesis_plus_gx` — shared by the four Sega systems, `pcsx-rearmed`, `flycast`, `ppsspp`, `melonds_ds`, `dolphin-emu` — shared by `gc` and `wii`), `game/<system>/<ROM name> [<ID>].toml` (same name
as the saves folder). Command-line
flags beat every file. All files are optional; a bad value is ignored with a warning.

```toml
[video]
shader = "crt-geom"      # none (default) | crt-easymode | crt-geom | crt-lottes | zfast-lcd | absolute .slangp path
scaling = "integer"      # aspect (default) | integer | stretch
[latency]
run_ahead_frames = 1     # 0 (default) | 1 | 2
[rewind]
enabled = true           # default false
buffer_mb = 64           # 16-512, default 64
[resume]
auto_save = true         # default true: save a resume point when you quit
mode = "ask"             # ask (default) | always | never (launcher only)
[achievements]           # global.toml only (ignored in system / core / game files)
enabled = true           # default true: achievements run while you are logged in
sound = true             # default true: unlock chime
leaderboard_popups = true  # default true
patched_rom_fallback = true  # default true: unknown patched ROM -> original ROM's achievements (softcore)
[input_keyboard]         # RetroPad buttons: b y select start up down left right a x l r l2 r2 l3 r3, stick directions
                         # lstick_up/down/left/right rstick_up/down/left/right (example swaps the default A/B keys)
a = ["KeyZ"]             # 0-2 physical keys (winit KeyCode names); [] = unbound
b = ["KeyX", "Space"]
[input_gamepad]          # positional names: south east west north back start
a = ["south"]            # left_shoulder right_shoulder left_trigger right_trigger
b = ["east"]             # left_stick right_stick dpad_up/down/left/right misc1 …
[hotkeys_keyboard]       # pause_menu save_state load_state prev_slot next_slot
save_state = ["F5"]      # toggle_fullscreen fast_forward_hold rewind_hold turbo_a_toggle
[hotkeys_gamepad]
save_state = ["right_stick"]   # R3 saves, L3 loads
load_state = ["left_stick"]
```

The launcher edits `global.toml` (Settings → Gameplay defaults) and the game file
(Detail → Settings; "Use default" removes the key); the pause menu's "Save for this
game" / "Save as default" write the same files. App writes keep unknown keys but
**drop comments** in the file they rewrite. The runner reads the files only at start
(last write wins). Detail → Save states lists `slotN` thumbnails with Load (starts the
game with `--load-state N`) and Delete (refused while the game runs); the resume point
comes first as a "Resume point" card (Continue / Delete).

### Controls and hotkeys

Edit them in the launcher: Settings → Controls (all games) or a game's Detail →
Settings → Controls (per game: each button or shortcut is "Default" or overridden).
Pick Keyboard or Gamepad, click Add and press the key or pad button (Esc cancels).
Each game button and each of the 8 shortcuts (pause menu, save / load state,
previous / next slot, fullscreen, fast-forward hold, rewind hold) takes up to two
inputs. An input already used on the same device moves to the new action ("Z removed
from B"), so the launcher never creates a conflict. Changes apply the next time a
game starts. Gamepad inputs are stored by position (south, east, …) and shown with
Xbox or PlayStation labels after you press a button on that pad; gamepad capture is
unavailable while a game runs. Game buttons are shared by every system; the editor
shows the buttons of one system at a time (NES, GB, GBC: B, A, Select, Start, D-pad; GBA adds
L and R; PlayStation has the full pad with L2 / R2 — Cross = B, Circle = A, Square = Y, Triangle = X;
Mega Drive rows read `A (Y)`, `B (B)`, `C (A)`, `X (L)`, `Y (X)`, `Z (R)`, `Mode (Select)`, Start — the
system's button name with the shared RetroPad name in brackets; Master System / Game Gear / SG-1000:
`1 (B)`, `2 (A)`, Start, on the Master System `Pause (Start)`; Nintendo 64 has 18 rows: `A (B)`, `B (Y)`,
`Z (L2)`, `L (L)`, `R (R)`, Start, D-pad, `Stick Up (Left stick ↑)` … and `C-Up (Right stick ↑)` … — the stick rows have no
gamepad default because the pad's own sticks feed the analog axes); Dreamcast has `A (B)`, `B (A)`, `X (Y)`, `Y (X)`, Start, D-pad, `LT (L2)`, `RT (R2)` and the 4 left-stick rows (the system's button name with the shared RetroPad name in brackets); PSP has `✕ (B)`, `○ (A)`, `□ (Y)`, `△ (X)`, Select, Start, D-pad, `L (L)`, `R (R)` and the 4 left-stick rows `Stick Up` … `Stick Right`; Nintendo DS has B, A, Y, X, Select, Start, D-pad, L, R, `Touch (stick cursor) (R2)`, `Microphone (blow) (L3)`, `Swap screens (R3)` and `Cursor Up` … `Cursor Right` (right stick). GameCube has A, B, X, Y, Start, D-pad, `Z (R)`, `L (L2)`, `R (R2)`, `L (light press) (L3)`, `R (light press) (R3)`, `Stick …` and `C-Stick …` (right stick); Wii has A, B, `Nunchuk C (X)`, `Nunchuk Z (Y)`, `1 (Start)`, `2 (Select)`, D-pad, `− (L)`, `+ (R)`, `Shake Nunchuk (L2)`, `Shake Remote (R2)`, `Home (R3)`, `Nunchuk Stick …` and `Tilt Remote …` (right stick). L3 / R3 (stick clicks) exist on the DS, GameCube and Wii; keyboard has no default for them. The Turbo A shortcut fires RetroPad A,
which is C on the Mega Drive, B on the Dreamcast, ○ on the PSP, A on the GameCube and Wii and nothing on the Nintendo 64.
L2 / R2 sit on the gamepad triggers by default. In a PlayStation, Dreamcast, GameCube or Wii game the triggers press L2 / R2 (GameCube: analog L / R; Wii: shakes) and
the fast-forward / rewind hold shortcuts give them up (silently, unless you rebound those shortcuts
yourself — then Controls shows a notice); on the Nintendo 64 the left trigger is Z (rewind is off there
anyway) and the right trigger keeps fast-forward; on the DS the right trigger is touch (R2) and gives up
fast-forward, the left trigger keeps rewind; in every other system (not PlayStation, Dreamcast, N64, DS, GameCube, Wii) the triggers keep fast-forward /
rewind (the PSP has no triggers; rewind is off there).

Fixed, never rebindable: Esc (keyboard) and Guide / PS (pad) open the pause menu and
act as Back in it; holding Select + Start for 0.5 s also opens it, so Back and Start
cannot be pad shortcuts (they stay usable as game buttons); menu navigation and
Alt+F4 are fixed too. Any key or pad input bound to "Pause menu" also closes the menu.
In hand-edited files a bad name, Esc / Guide or more than two inputs is ignored with a
warning; an input used by both a shortcut and a game button goes to the shortcut
(warning in Settings → Controls). "Reset controls to defaults" removes your bindings
from that file. The launcher window cannot capture keys that WebView2 keeps for itself
(F10, Alt, the Windows key): bind those by editing `global.toml`.

### Resume

When you quit a game (pause menu Quit, closing the window or Alt+F4, even while
paused) after about 10 seconds of play (600 emulated frames), the runner saves a resume point. A crash,
a kill, an error or a shorter session keeps the previous one. On the next Play the
launcher asks **Continue** or **Start fresh** (`resume.mode = "ask"`); `always`
continues without asking (unless there is a warning), `never` always starts normally. Start fresh keeps your
in-game saves and the resume point. The dialog warns, and focuses Start fresh, when
the in-game save (`save.srm`) is newer than the resume point, the patch file changed,
or the core version changed. A resume point the game cannot load (GameCube / Wii: made by another Dolphin
version, or on another disc or revision of the game) shows the reason and only Start fresh. Turn saving off with `resume.auto_save = false`
(Settings → Gameplay defaults, or per game).

### RetroAchievements

[RetroAchievements](https://retroachievements.org) (RA) works on 14 systems: NES, SNES, Game Boy, Game Boy
Color, Game Boy Advance, Mega Drive / Genesis, Master System, Game Gear, SG-1000, PlayStation, Nintendo 64,
Dreamcast, PSP and Nintendo DS. Built on rcheevos v12.5.0 (the library RetroArch uses).

- **Log in once**: Settings → RetroAchievements → username + password. The password is used once to get a
  login token and is never stored (see [Privacy](#privacy): the token file is **not encrypted**). Log out
  deletes it. When RA rejects the token later the game shows "please log in again in Settings" and the
  launcher shows you logged out. Settings also has toggles for achievements on / off, the unlock sound, leaderboard pop-ups
  and "Use original ROM achievements for patched games (softcore)" (`[achievements]` in `global.toml`, global only).
- **Softcore only for now**: every unlock is a softcore unlock; save states, rewind, run-ahead, fast-forward
  and pause stay free. Hardcore is fully built (state loads refused, rewind off, launcher Continue / slot Load
  hidden, pause rate limited, disallowed core options drop to softcore) but hidden until RetroAchievements
  approves Nostyra as an emulator (RA asks for about 6 months of public releases first; then contact RAdmin
  for user-agent validation and flip `achievements::HARDCORE_UI_ENABLED`).
- **Leaderboards**: rcheevos v12.5.0 runs leaderboards in hardcore only, so with softcore there are no
  leaderboard attempts, trackers or submissions in this version (the code and tests exist for hardcore).
- **In game**: a placard when the set loads ("N of M unlocked", softcore), an unlock pop-up with the badge and
  a short chime, mastery and progress / challenge indicators, all drawn on the overlay above the shader. The
  pause menu gains **Achievements** (list with badges, locked / unlocked, progress). Badges are downloaded once
  into `<data>/cache/achievement-badges/` (grey square until they arrive).
- **Launcher**: Detail → **Achievements** shows the game's list, badges, points and unlock times from the last
  time you played it while logged in ("Play this game once while logged in to load its achievements" before
  that).
- **Offline**: an unlock that cannot be sent is kept in `<data>/achievements/pending/` and sent the next time
  you play a game or start the launcher. Save states and the rewind buffer carry achievement progress, so
  loading a state also restores hit counts.
- **Playing offline**: a game you played **online at least once** (same account) also works with no network.
  The runner answers from the last server answers it saved (login, the game's set and your unlocks) and shows
  "RetroAchievements: offline — unlocks will sync when online"; the session is softcore; unlocks wait in
  `pending/` and are sent when the network is back (the next game or launcher start; if it comes back during
  the game they are retried at once, not yet verified with the real server), with their original time. Achievements already earned (online, offline or synced since)
  do not pop again. Detecting "offline" waits for the connection timeout, so the set loads about 10–20 s
  after an offline start; achievements triggered before that are missed. A game never played online shows
  "RetroAchievements: offline: play once online first" and has no achievements that session.
- **Patched games**: a soft-patched cartridge (`.ips` / `.bps` / `.ups` beside the ROM) is identified by the
  patched bytes. Translations registered on RA load their own set directly. If RA does not know the patched
  game, Nostyra retries once with the original ROM's hash and, when that works, shows "RetroAchievements:
  patched ROM not recognised — using the original ROM's achievements (softcore)" (always softcore; on by
  default, Settings → RetroAchievements → "Use original ROM achievements for patched games (softcore)",
  `[achievements] patched_rom_fallback`). Achievements may misfire if the patch changes game memory (text
  engines, expanded ROMs): wrong or missed unlocks go to your account (reset them on the RA site). Whether RA's
  rules allow this for patched games is not verified; turn the toggle off if unsure. Off, or if the original
  is unknown too: "Unknown game — achievements inactive" with the patched hash.
- **Identification**: the game file is hashed the way RA does (cartridges: the patched bytes the core runs;
  discs: through Nostyra's own CHD / CSO / CDI / GDI / cue readers). A game RA does not know shows "Unknown
  game — achievements inactive" with its hash. `.m3u` games use the first disc; a disc swap re-identifies.
- **Known limits**:
  - PlayStation `EBOOT.PBP` games: "achievements not available for this format" (rcheevos cannot hash PS1
    eboots; use the `.cue` / `.chd`).
  - GameCube and Wii: not supported (Dolphin's libretro core has no RA support; the tab says "RetroAchievements
    is not available for …").
  - Titles in Chinese, Japanese or Korean show boxes (□) for those characters: the bundled font covers Latin,
    Greek and Cyrillic only.
  - Copier `.smd` Mega Drive dumps are not recognised by RA (it knows the raw `.md` / `.gen` dumps).
  - SNES SA-1 games: achievements on the SA-1's own RAM may not work (Snes9x exposes no memory map; Nostyra
    reads its main RAM and save RAM).
  - The launcher shows your points only right after logging in (not after a restart).
  - Offline: a disc swap keeps the previous disc's set (the new disc is not looked up); the cached set is the
    one from your last online session (revisions on RA apply after the next online start); the patched-game
    fallback works offline only if the original game was played online before.
  - Not yet: hardcore (above), rich presence shown in the launcher, a live achievement list in the launcher
    without playing, RAIntegration (set development), encrypted token storage, CJK font.

### PlayStation

Played by **PCSX ReARMed** (offered for download on the first Play, or Settings → Emulator cores).

- **Game files**: Redump `.cue` + `.bin` tracks (keep them together; the `.cue` is the game, its track
  files are hidden from the library), a single `.bin` or `.iso`, `.chd`, PS1 classic `EBOOT.PBP`. A
  `.bin` whose `.cue` is missing still plays, but a multi-track game then has no CD music. A cue whose
  `FILE` line names a missing file is not listed: the scan notice "N files not added — Details" (and
  Settings → ROM folders) shows the reason ("track file 'X' not found — fix the FILE line or rename
  the file").
- **Multi-disc games**: write an `.m3u` beside the discs, one disc per line (relative paths), e.g.
  `Final Fantasy VII.m3u` holding `Final Fantasy VII (Disc 1).cue`, `… (Disc 2).cue`, `… (Disc 3).cue`.
  The playlist is one game (its discs are hidden) and the pause menu gains **Disc** to swap discs; save
  states remember the inserted disc.
- **Identification**: the first data track's SHA1 against the Redump DAT gives the official title; a
  disc that matches only by its boot serial (translations, hacks, other dumps) keeps its file name as
  title but gets the DAT's region, artwork and metadata. `.chd` / `.pbp` match by serial only.
- **BIOS**: not needed — the core's built-in HLE BIOS runs most games. Some games need a real BIOS:
  Settings → BIOS → **Add BIOS file…** copies a BIOS you dumped from your own console into
  `<data>/system/` (known dumps get the name the core looks for, e.g. `scph1001.bin`) and shows what
  each region (JP / US / EU) boots. One BIOS of any region is used for every disc. Nostyra never
  includes or downloads BIOS files.
- **Memory cards**: card 1 is the game's `save.srm` (128 KiB raw card, same format as ePSXe `.mcr`,
  DuckStation `.mcd`), card 2 is `pcsx-card2.mcd`, both in `saves/psx/<game>/` (one pair per game, all
  discs of an `.m3u` share it). Detail → Save states → Memory cards → **Import card…** brings a card
  from another emulator (`.mcr`, `.mcd`, `.srm`, `.gme`, `.vgs`); the previous card is kept as
  `<file>.before-import-<date>`. ePSXe "memory card per game" files are
  `memcards/games/<SERIAL>-00.mcr` (slot 1) and `-01.mcr` (slot 2). Loading a save state also restores
  the memory card stored in it (as in other emulators): save in game before loading an old state.
- Not yet: `.img` / `.ccd` / `.mdf` / `.ecm`, PPF patches, analog sticks / DualShock / rumble, core
  options, automatic grouping of "(Disc N)" files without an `.m3u`.

### Sega (Mega Drive, Master System, Game Gear, SG-1000)

Played by **Genesis Plus GX** (offered for download on the first Play, or Settings → Emulator cores;
non-commercial licence, never bundled). No BIOS is needed.

- **Game files**: Mega Drive / Genesis `.md` (No-Intro), `.gen`, `.smd` (copier dumps) and `.bin`;
  Master System `.sms`, Game Gear `.gg`, SG-1000 `.sg`. Keep the 8-bit extensions: the core picks the
  system by extension, so a Master System game renamed `.bin` would start as a Mega Drive game.
- **`.md` and `.bin` need the `SEGA` header** (at 0x100, as every licensed cartridge has): a `.md`
  without it is ignored like any Markdown document (a `README.md` in your ROM folder is not a game), a
  `.bin` without it is tried as a PlayStation disc and otherwise listed as not scanned. An odd dump
  without the header still plays when renamed `.gen`. A Mega Drive `.bin` beside an unrelated
  same-name `.cue` stays a game.
- **Identification**: `.smd` files are de-interleaved before hashing, so they match the No-Intro entry
  of the raw dump and share its saves. 32X games (`.bin` with `SEGA 32X`) are listed as Mega Drive but
  do not run (Genesis Plus GX has no 32X); Sega CD discs are not supported yet.
- **Saves**: battery RAM / EEPROM in `save.srm`; the core saves up to the last used byte, so the file
  may be small (a few bytes for some games). A game that never touched its save memory writes none.
- **Soft patches** (`.ips` / `.ups` / `.bps` beside the ROM) work: the patched ROM is handed to the
  core as a short temporary file. Patches are made for raw dumps — patching an `.smd` breaks the game.
  ROM paths of 256 characters or more play the same way (the core cannot open such paths itself).
- **Pads**: 6-button games get X / Y / Z / Mode automatically (from the cartridge header); other games
  use a 3-button pad.
- Not yet: Sega CD, 32X, Pico, light guns / mouse / multitap, core options (region, overscan, FM sound
  chip), `.68k` / `.sgd` / `.mdx` / `.bms`.

### Nintendo 64

Played by **Mupen64Plus-Next** (GPL-2.0-or-later; offered for download on the first Play, or Settings → Emulator
cores). No BIOS is needed.

- **Needs OpenGL 3.3**: the core draws with the GPU (GLideN64) into a hidden OpenGL context; the picture is read
  back and shown like every other system (shaders, thumbnails and fullscreen work the same). A PC without OpenGL
  3.3 gets "This system needs OpenGL 3.3 graphics — update the graphics driver." (runner exit code 6).
- **Game files**: `.z64` (No-Intro, big-endian), `.v64` (byte-swapped) and `.n64` (little-endian), 4 KiB to
  64 MiB. The first four bytes decide the byte order, not the extension; a file without an N64 header is listed as
  not scanned ("missing N64 header"). Every byte order hashes as the `.z64` image, so copies in different orders
  match the same DAT entry and share one save folder. `.bin`, `.u1`, `.ndd` (64DD) and archives are not listed.
- **Picture**: rendered at a fixed 960×720 (4:3); the window opens 720 px high (at most 90 % of the screen).
- **Saves**: one `save.srm` (296 960 bytes, the same layout as RetroArch) holds EEPROM, SRAM, Flash and the
  Controller Pak (memory pak) in port 1. Save states work (about 16 MiB each); **run-ahead and rewind are off**
  for the Nintendo 64 (the core's states are not frame-exact; the pause menu says so). A state file of the wrong
  size is refused; a damaged state of the right size can hang the game (close the window and start fresh).
  Pause menu → Reset game takes about a second (the console resets through its NMI).
- **Pads**: left stick = N64 stick, right stick = C-buttons, left trigger = Z, A / B on the pad's south / west
  buttons, right trigger keeps fast-forward. The core applies a 15 % stick deadzone (no extra one in Nostyra).
  Keyboard: Z = A, A = B, E = Z, Q / W = L / R, I/J/K/L = stick, T/F/G/H = C-buttons. Turbo A has no effect (the N64 pad has no RetroPad A).
- **Soft patches** are applied to the file as it is: most patches are made for `.z64` dumps, so patch a `.z64`
  (a `.z64` patch on a `.v64` / `.n64` breaks the game).
- The core keeps its settings file and the GLideN64 shader cache in `<data>/system/Mupen64plus/`.
- Not yet: rumble pak, transfer pak, 64DD, texture packs, resolution / widescreen / core options, more than one
  player, Vulkan (ParaLLEl-RDP).

### Sega Dreamcast

Played by **Flycast** (GPL-2.0-or-later; offered for download on the first Play, or Settings → Emulator cores).
No BIOS is needed: Flycast's built-in HLE BIOS boots most games.

- **Needs OpenGL 3.3**, like the Nintendo 64 (same message and exit code 6 without it). The picture is rendered
  at 1280×960 (2× the console) and read back like every other system.
- **Game files**: TOSEC `.gdi` + track files, Redump `.cue` + `(Track N).bin` files, `.chd` (GD-ROM or CD),
  DiscJuggler `.cdi` (MIL-CD / self-boot) and `.m3u` playlists. A disc is recognised by its IP.BIN (`SEGA
  SEGAKATANA`), never by its extension: a `.cue` / `.chd` / `.m3u` of a PlayStation game stays PlayStation. Track
  files are hidden; a missing track file is listed under "files not added" with its name. `.bin` alone (Flycast
  would treat it as an arcade ROM), `.elf`, Naomi / Atomiswave archives are not listed.
- **Identification**: the serial is IP.BIN's product number (`MK-51064`, `HDR-0080`, `T-8101N`), matched against
  the Redump DAT under every form it uses (`MK-51064` finds the US entry `51064`). A Redump `.cue` is identified
  by its largest data track (the one the DAT hashes); `.gdi` dumps usually match by serial; `.chd` / `.cdi` hash
  the whole file. The first scan hashes the largest data track of each disc (about 1 GB per game).
- **BIOS**: Settings → BIOS → Sega Dreamcast → **Add BIOS file…** takes your own `dc_boot.bin` (2 MiB, known
  checksum) and stores it as `<data>/system/dc/dc_boot.bin`. Games then boot through the real BIOS (boot
  animation); any other file there is ignored and the HLE BIOS runs (Flycast would crash on a wrong file).
- **Saves**: VMU memory cards live in the game's save folder (`<product>.A1.bin`, one 128 KiB file per pad), so
  every game has its own cards and a multi-disc game shares them. The console's flash (`dc_nvmem.bin`) is shared in
  `<data>/system/dc/`. Save states work (36 MiB each, a short pause while saving); **run-ahead and rewind are
  off** (the pause menu says why). A damaged state is refused and the game continues where it was.
- **Pads**: the pad's south / east / west / north buttons are Dreamcast A / B / X / Y, LT / RT are the analog
  triggers (pressure), the left stick is the analog stick (Flycast's 15 % deadzone). Keyboard: Z = A, X = B,
  A = X, S = Y, E / R = full LT / RT, I/J/K/L = stick, Enter = Start. On the pad both triggers are game buttons,
  so fast-forward / rewind hold stay on the keyboard.
- **Multi-disc**: put the discs in an `.m3u` (one row); pause menu → Disc swaps when the game asks. Reset game
  reboots the first disc of the playlist.
- Not yet: Naomi / Atomiswave, rumble (Purupuru), VMU screen and manager, keyboard / mouse / light gun, more than
  two pads, internal resolution / widescreen / core options, Vulkan.

### Sony PSP

Played by **PPSSPP** (GPL-2.0-or-later; offered for download on the first Play, or Settings → Emulator cores).
No BIOS is needed. Installing the core also downloads its system files (fonts, dialog images, compatibility
table; one progress bar, all or nothing); the runner copies them into `<data>/system/PPSSPP/` before each start
(again when a re-downloaded core brings new ones). Without them the game does not start: "PPSSPP system files are
missing (…). Re-download the core in Settings → Cores." (exit code 3).

- **Needs OpenGL 3.3**, like the Nintendo 64 and Dreamcast (same message and exit code 6 without it). The
  picture is rendered at 960×544 (2× the PSP screen), shown 16:9 (the core crops to 960×540) and read back like every other system.
- **Game files**: UMD dumps as `.iso`, `.cso` (CISO v1) or `.chd` (made with `chdman createdvd`), and
  `EBOOT.PBP` (PSN games, homebrew), up to 8 GiB. A UMD is recognised by its content (`PSP GAME` volume or
  `PSP_GAME` folder), never by its extension: a PlayStation `.iso` / `.chd` / PS1 classic `EBOOT.PBP` stays
  PlayStation. UMD Video / Audio discs, ZSO and CSO v2 files are listed under "files not added" with the reason.
  `.m3u` playlists are not used for the PSP.
- **Identification**: an `.iso` is hashed whole against the Redump DAT; `.cso`, `.chd` and eboots match by serial
  (`UMD_DATA.BIN` or `PARAM.SFO` `DISC_ID`, e.g. `ULUS10041` finds `ULUS-10041`). An eboot without a DAT match is
  titled from its `PARAM.SFO` (else its folder name). Covers come from libretro-thumbnails like every system.
- **Saves**: the game's save folder is its memory stick: in-game saves in `saves/psp/<game> [<ID>]/PSP/SAVEDATA/`,
  updates / DLC in `PSP/GAME/<DISC_ID>/`; there is no `save.srm`. Every eboot's folder is named `EBOOT [<ID>]`. To
  import a save into a sequel or share DLC, copy the folder between the games' `PSP/SAVEDATA` yourself. Save states
  work (about 40–72 MiB each); **run-ahead and rewind are off** (the pause menu says why). The game boots for a
  moment after start: Continue waits for it (up to 10 s), loading a slot before that shows "Core not ready", and a
  bad resume point says "Could not resume".
- **Pads**: south / east / west / north = ✕ / ○ / □ / △, shoulders = L / R, the left stick is the analog nub
  (PPSSPP's 15 % deadzone). Keyboard: Z = ✕, X = ○, A = □, S = △, Q / W = L / R, I/J/K/L = stick, Enter = Start,
  Right Shift = Select. The triggers keep fast-forward / rewind hold.
- **Errors**: a disc that is not a PSP game started with `--system psp` fails with exit code 4; when the core stops
  (e.g. an unreadable game) the runner ends with exit code 5 and the core's last error line as the reason.
- Not yet: a memory stick shared by all games, PSN DAT, ZSO / CSO v2 / DAX, UMD Video, multi-UMD playlists,
  texture packs, cheats, ad-hoc / network play, internal resolution / core options, Vulkan / D3D11.

### Nintendo DS

Played by **melonDS DS** (GPL-3.0-or-later; offered for download on the first Play, or Settings → Emulator
cores; `scripts/fetch-melondsds-core.*` for development). No BIOS or firmware is needed: the core uses its
built-in FreeBIOS and generated firmware (language English, user name "melonDS DS") and boots the game directly.
The picture is drawn by the software renderer, so no OpenGL driver is needed.

- **Decrypted dumps only**: the built-in BIOS cannot boot a dump whose secure area is still encrypted. Use the
  No-Intro "Decrypted" set (what the bundled DAT lists). An encrypted dump is listed in the library (it is a DS
  game) but Play stops with "This DS dump is encrypted; Nostyra plays decrypted dumps (No-Intro Decrypted)"
  (exit code 4). When the core itself rejects a game, the error carries the core's reason (e.g. "ROM isn't valid,
  did you select the right file?").
- **Game files**: `.nds`, `.dsi` (DS games with DSi extras, played in DS mode) and `.ids` (iQue DS), 4 KiB to
  512 MiB, recognised by the DS header checksum, never by the folder. DSi-only games are listed under "files not
  added" with "DSi-only game — not supported"; a file with a broken header gets its own reason. Zip / 7z
  archives are not read: extract them first.
- **Identification**: the whole file is hashed against the No-Intro DS (Decrypted) DAT (files over 32 MiB are
  hashed in a stream, never read whole); a dump the DAT does not know (hack, translation, trimmed dump) is
  matched by its 4-character game code (e.g. `AMCE`) and keeps its file name as the title. Covers come from
  libretro-thumbnails like every system.
- **Screens**: both screens in one picture, top over bottom (256×384, the window opens at 480×720). R3 swaps to
  side by side (512×192) and back; a normal window resizes itself to the new shape, fullscreen and maximised
  windows just re-fit the picture.
- **Touch screen**: with the mouse — left button over the bottom screen touches it (presses on the top screen are
  ignored, dragging off the bottom screen or out of the window releases); the OS cursor stays visible and the
  core draws its own while you touch. On a pad the right stick moves a touch cursor and R2 touches (whichever of
  mouse and stick you used last wins). Keyboard: T/F/G/H move the cursor, R touches.
- **Pads**: the DS buttons sit on the SNES positions (B, A, Y, X, L, R, Select, Start), the left stick is also the
  D-pad. L3 (press the left stick) holds "blow into the microphone"; R3 swaps the screens. The right trigger is
  touch, so on the DS it no longer fast-forwards (the keyboard hold still does); the left trigger keeps rewind.
  The lid button is not mapped.
- **Saves**: the in-game save (EEPROM / flash / NAND) is `saves/nds/<game> [<ID>]/save.srm`, written every
  minute and on exit; homebrew without a save chip has none. To bring a save from another emulator, copy a raw
  `.sav` (melonDS, no `.dsv` footer) into the game's folder and rename it `save.srm`. The core also writes
  `<data>/system/melonDS DS/wfcsettings.bin` and an empty `melonDS DS/` folder in each game's save folder; both
  are harmless. The in-game clock follows the PC's clock.
- **Save states** work (about 18–20 MiB each, exact size per game; a state of another size is refused).
  **Loading a state also rewrites the in-game save** with the one stored in the state (core design), so a
  newer in-game save made after that state is lost; keep that in mind before loading an old slot. Rewind works;
  run-ahead works but costs about 18 ms per frame (dropped frames on most PCs; leave it off). The stick touch
  cursor is not part of a state: after a load it stays where it was, and with run-ahead it moves faster.
- **Memory**: the core keeps several copies of the game in memory, so a 512 MiB game needs about 2 GB of RAM.
- Not yet: OpenGL renderer with up-scaling, your own BIOS / firmware (for encrypted dumps), DSi mode and
  DSiWare, GBA slot-2 games, a real microphone, rumble / sensors, Wi-Fi, the lid button, rotated / hybrid screen
  layouts, archives, Download Play / DSi DATs, `.dsv` save conversion, core options.

### Nintendo GameCube and Wii

Played by **Dolphin** (GPL-2.0-or-later; offered for download on the first Play, or Settings → Emulator cores;
`scripts/fetch-dolphin-core.*` for development). Installing the core also downloads Dolphin's system files
(`Dolphin.zip`, one progress bar, all or nothing); the runner copies them into `<data>/system/dolphin-emu/Sys/`
before each start, as for the PSP. No BIOS / IPL is needed: Dolphin boots discs with its built-in replacement
(no GameCube boot animation) and has the Wii keys built in.

- **Needs OpenGL 3.3**, like the Nintendo 64, Dreamcast and PSP (same message and exit code 6 without it). Without
  it Dolphin would silently run with a black picture, so Nostyra stops with the message instead. The picture is
  rendered at the console's own size (1×, 640×528) and read back like every other system; a Wii game set to 16:9
  shows 16:9 and a normal window resizes to the new shape. Dolphin gets exactly OpenGL 3.3, so effects that need
  newer OpenGL (bounding box, GPU texture decoding) are missing: Paper Mario: The Thousand-Year Door / Super Paper
  Mario-type games can show glitches.
- **One CPU thread**: Dolphin runs single-core (CPU and graphics emulation on one thread). Demanding Wii games may
  not reach full speed on slower CPUs.
- **Game files**: GameCube `.iso`, `.gcm`, `.ciso`, `.gcz`, `.wia`, `.rvz`, homebrew `.dol` / `.elf` and `.m3u`
  playlists (up to 2 GiB); Wii `.iso`, `.ciso`, `.gcz`, `.wia`, `.rvz`, `.wbfs`, `.dol` / `.elf` (up to 8 GiB).
  Only the disc header is read (also inside compressed files): its magic decides GameCube or Wii, never the
  extension, so a PlayStation, PSP or PS2 `.iso` is never taken for a GameCube / Wii disc (a PS2 `.iso` stays one
  "files not added" row, as before). A `.dol` / `.elf` with Wii-only code is a Wii program. An `.elf` that is not a
  PowerPC program (PSP, Dreamcast, PC) is skipped quietly. Not read: `.wad` (WiiWare / Virtual Console), `.tgc`,
  NKit, CHD, archives.
- **Identification**: by the 6-character game ID in the disc header (e.g. `GALE01`), matched against the Redump
  GameCube / Wii DAT by its 4-character code (`GALE` finds `DL-DOL-GALE-USA`). Discs are never hashed whole (1.4–8
  GB each), so the first scan stays fast; `.dol` / `.elf` files are hashed (small). The same game as `.iso` and
  `.rvz` is one game (one content id, one save folder). Covers come from libretro-thumbnails like every system.
- **Saves**: Dolphin keeps its own `User/` folder inside the game's save folder (`saves/gc/<game> [<ID>]/User/`,
  `saves/wii/…`): GameCube memory card saves (`User/GC/<region>/Card A/`, one file per save), the Wii NAND
  (`User/Wii/`) and the shader cache (`User/Cache/`, can reach tens of MB per game). There is no `save.srm`. Every
  game has its own memory card and NAND, so saves another game would read (Pokémon Colosseum / XD bonuses, Smash
  Bros. Brawl stages, Miis) are not visible across games.
- **Save states** work (about 90–120 MiB each, at most 256 MiB; saving pauses briefly); **run-ahead and rewind
  are off** (the pause menu says why). Continue waits for the game to boot. A state only loads in the Dolphin
  version that made it: after a core update the launcher hides Continue and marks old slots "made by another
  Dolphin version" (they cannot be loaded). A damaged or truncated state is refused before Dolphin sees it.
- **GameCube pad**: the GameCube buttons sit on the SNES positions (A, B, X, Y), the left stick is the control
  stick, the right stick the C-stick, LT / RT are the analog L / R triggers (pressure) and RB is Z; L3 / R3 click
  L / R digitally ("light press"). Both triggers are game buttons, so the pad's fast-forward / rewind holds give
  them up (the keyboard holds stay).
- **Wii Remote + Nunchuk**: every Wii game gets a Wii Remote with a Nunchuk. A / B on the pad's A / B, Nunchuk
  C / Z on X / Y, − / + on LB / RB, 1 / 2 on Start / Select, Home on R3, shake Nunchuk / Remote on LT / RT, left
  stick = Nunchuk stick, right stick = tilt the Remote. Controls shows every row with its Wii name.
- **Pointer (Wii Remote IR) = the mouse**: the game's own pointer follows the mouse over the picture (the whole
  picture is the TV the Remote points at). The OS cursor is hidden over the picture (the game draws its own) and
  comes back off the picture and while the pause menu is open. Mouse buttons do nothing: press A / B on the pad or
  keyboard. When the mouse leaves the picture the pointer stays at the nearest picture edge (Dolphin ignores
  "pointing off screen"). The game's pointer may not sit exactly under the mouse (Dolphin's default pointer angles).
  Only player 1 has a pointer; a pad-only player cannot aim (the right stick tilts the Remote).
- **HOME menu**: do not choose "Wii Menu" there: there is no Wii System Menu and the game hangs (Dolphin issue
  #508). Quit from Nostyra's pause menu instead.
- **Multi-disc (GameCube)**: put the discs in an `.m3u` (one row, identified by disc 1); pause menu → Disc swaps
  when the game asks.
- **Known limits**:
  - All discs and revisions of a game share its game ID, and so one save folder (memory card, NAND, shader cache):
    a loose disc-2 row sees disc 1's memory card save. A standalone disc-2 or Rev 1 row shows the name of the
    first DAT entry with that ID (usually disc 1 / Rev 0); a few games share one ID across releases the same way.
  - Save states stay per disc and revision: a state made on another disc or revision of the same game is refused
    ("made from another disc or revision"; the `.m3u` row has its own). Each save folder holds one resume point,
    so quitting disc 1 replaces disc 2's resume point (the disc-2 row then offers only Start fresh).
  - Mods, translations and hacks that keep the retail game ID share the save folder, memory card and shader cache
    with the retail game.
  - Moving several discs or revisions of one game to another folder at once can relink a file onto a sibling's
    row (they share one content id): play history / favourites may swap rows; saves are unaffected.
  - Windows CI runs no Dolphin save-state test (no OpenGL there); those run on Linux CI (llvmpipe) and on a PC
    with a GPU.
- Not yet: dual-core CPU, Vulkan / D3D, newer OpenGL for Dolphin (bounding box), 2× internal resolution /
  upscaling / widescreen-hack settings, your own IPL, a memory card / NAND shared by all games, 4 players,
  GameCube pad in Wii games, Classic Controller / sideways Remote, aiming with the right stick, mouse buttons as
  A / B, a pointer for player 2, motion / rumble, real Wii Remotes, `.wad`, `.tgc`, NKit, cheats, Game Boy Advance
  link, state compression, core options.

## Releasing
Maintainers: see [docs/deployment-guide.md](docs/deployment-guide.md) (`dev` → `main` one commit per release, `scripts/release.ps1`).
