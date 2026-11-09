# Nostyra 0.5.0 — release notes

> **Old Worlds. New Ways.**

Windows x64 portable build: `Nostyra-0.5.0-windows-x64-portable.zip`. Upgrading from 0.4.0: extract the new zip over
the old folder (or copy your `data\` folder into the new one); your library, saves and settings carry over. 0.5.0 makes
no library database change, so 0.4.0 still opens the same `data\` folder. Upgrading from 0.2.0 or 0.3.x: the first start
upgrades the library database (as in 0.4.0); older versions cannot open it afterwards, so keep a copy of `data\` if you
may want to go back.

Nostyra is a free, open-source (GPL-3.0-or-later) retro game launcher and libretro frontend. Point it at
your ROM folder, organised however you like, and it builds a library with covers and metadata. Games
run through libretro emulator cores, which Nostyra downloads the first time you play a system.

This is an early release. SNES is the only system tested end to end so far. The other systems pass
automated tests but are marked **Experimental** until they get manual testing.

## What's new in 0.5.0

- **Cloud saves (desktop, Experimental)**: Settings → **Cloud saves** keeps each game's saves in step across your PCs.
  - **Google Drive**: click **Sign in**, approve in your browser, done. Nostyra only gets access to the files it
    creates itself (Google's `drive.file` permission), in a visible **Nostyra** folder in My Drive; it cannot see
    the rest of your Drive. **Sign out** removes the access from this PC and keeps your saves in Drive.
  - **Local folder**: choose a folder that your OneDrive or Dropbox desktop app (or a network share / USB drive)
    already syncs, on every PC. Nostyra writes its copy under `<folder>\nostyra-cloud\`.
  - Direct **OneDrive** and **Dropbox** sign-in is listed but shows "not available in this version yet"; it comes
    in a later release (use the Local folder with their desktop apps meanwhile).
  - **When it syncs**: before a game starts (download only), after you quit it (upload), in the background when
    Nostyra starts (**Sync when Nostyra starts**) and on **Sync now**. Games never wait more than about 15 seconds:
    offline or if the cloud is unreachable, the game starts with this PC's saves and they upload later.
  - **What syncs**: everything in the game's save folder (battery saves, memory cards, VMUs, PSP / GameCube / Wii
    saves). Save states and the resume point are included unless you untick **Also sync save states**. PlayStation 2
    memory cards are shared by all PS2 games and sync with any of them. Per-game settings do not sync. Games are
    matched by their content, so a renamed ROM on another PC still gets its saves. Files over 256 MiB are skipped.
  - **Never a silent overwrite**: when a save changed on both PCs, Nostyra asks: **This PC**, **Cloud** or
    **Play without syncing**. The copy you don't pick is kept in `data\cloud\backups\` (last 5 per game). The first
    sync of a library with many differences offers one choice for all games.
  - The game Detail page shows a small status chip (Synced, Upload pending, Saves differ, The last sync failed).
- **Update notice**: when a newer release with the Windows zip is out, a banner offers **Open release page**,
  **Skip this version** or **Later**. **Settings → Updates** shows your version, a **Check for updates at startup**
  toggle and **Check now**. It only tells you; download and extract the new zip yourself (see Privacy).

## What's new in 0.4.0

Four new systems: **Sony PlayStation 2**, **Atari 2600**, **Atari 7800** and **Atari Lynx** (20 systems in all).
They pass automated tests and ran on real games during development, but are **Experimental** like the other
systems until they get manual testing. Rescan your ROM folders once (Settings → ROM folders) to pick up the new
file types.

- **Sony PlayStation 2** (desktop only) on **PCEE2**, the PCSX2 libretro core (PCSX2 v2.9, OpenGL).
  - **Needs a BIOS dumped from your own PS2** — there is no built-in replacement and Nostyra never includes or
    downloads one. Press Play on a PS2 game and Nostyra asks for the file (**Choose BIOS file…**: `.BIN` or
    `.ROM0` under any name; the `.NVM` / `.MEC` / `.ROM1` files beside it are copied along), then starts the game.
    Settings → BIOS lists your dumps with version, region and date ("v1.60 · USA · 2002-02-07") and shows which
    one each disc region boots.
  - Game files: `.iso`, `.bin` + `.cue`, `.chd`, `.cso`, `.zso` and `.m3u` multi-disc playlists, up to dual-layer
    DVD size. Games are identified by the disc serial against the Redump database, so a scan never reads a whole
    multi-GB disc.
  - Memory cards are shared by every game, as on a real console (`data\system\pcsx2\memcards\Mcd001.ps2` and
    `Mcd002.ps2`). Save states work (96 MiB each); run-ahead and rewind are off. DualShock 2 with both sticks.
    RetroAchievements supported.
  - Needs **OpenGL 3.3** for the normal path (without it the core falls back to its own slower readback path)
    and a fast CPU: full speed needs roughly a 4-core Haswell-class CPU or better. Native resolution only.
- **Atari 2600** (Stella), **Atari 7800** (ProSystem) and **Atari Lynx** (Handy). No BIOS needed. `.a26`,
  `.a78` (with or without header), `.lnx` / `.lyx` / `.o`, and loose `.bin` files when the No-Intro database
  knows them. Save states, rewind and run-ahead work; the Lynx EEPROM is saved in the game's save folder; vertical
  Lynx games rotate automatically. Console switches live on the pad (2600: Select = Game Select, Start = Game
  Reset, L / R = difficulty; 7800: X = Reset, Start = Pause; Lynx: L / R = Option 1 / 2). RetroAchievements
  supported. Also playable in **Nostyra Online**.
- **RetroAchievements** now covers 18 systems.

## What's new in 0.3.2

- **Nostyra Online: the keyboard works on macOS.** On a Mac the game started but ignored the keyboard,
  because keyboard focus stayed on the launcher page instead of the game. The game now takes focus when it
  starts and whenever you click it, and keys that still reach the page are passed on to the game. The
  desktop app is unchanged.

## What's new in 0.3.1

- **Nostyra Online moved to https://nostyra.com/play** (it was at `/online`). Old links and bookmarks
  redirect to the new address. Your library, browser-storage saves, remembered folders and settings stay:
  the browser keeps them per site, not per address. The desktop app is unchanged.

## What's new in 0.3.0

- **Nostyra Online** — the same launcher in your browser at **https://nostyra.com/play** (desktop
  **Chrome or Edge**). Choose your ROM folder and play NES, SNES, Game Boy / Color / Advance, Mega Drive,
  Master System, Game Gear, SG-1000, PlayStation and (since 0.4.0) Atari 2600 / 7800 / Lynx right in the page
  (EmulatorJS 4.2.3). Nothing is
  uploaded: your ROMs and saves stay on your computer.
  - **Shares the desktop Data folder**: point the web edition at the desktop app's Data folder and both use
    the same in-game saves (`save.srm`), settings, controls and BIOS files. Save states stay separate (the
    browser keeps its own `web-slot0..9` / `web-auto`), and browser play time is added to the desktop history.
  - Or keep everything in **browser storage** (this browser profile only).
  - Desktop-only in the browser: core downloads, shaders, run-ahead, rewind, RetroAchievements, logs and bug
    reports, memory card import. N64, DS, PSP, Dreamcast, PlayStation 2, GameCube and Wii need the desktop app.
- **New installs keep their data in `Documents\Nostyra`** (browsers cannot open folders inside AppData).
  Existing installs keep their folder; portable copies are unchanged. Settings → Data folder shows a hint when
  your folder is inside AppData and you want to share it with the browser.
- **ZIP archives**: opt-in scanning and playing of single-ROM `.zip` files (NES, SNES, Sega 8/16-bit, GBA) in
  Settings → ROM folders. Contributed by **jnovax (Lam Nguyen)** in PR #1 — thank you!

## Features

- **Library and launcher**: add one or more ROM folders. Each file is classified by its content, never by
  its folder name. Moved or renamed ROMs keep their play time, favourite, edited metadata, saves and
  per-game settings. Home, Library (search, system / genre filters, sort), game Detail pages, play
  time tracking and favourites.
- **Metadata and artwork**: games are identified against the bundled No-Intro / Redump DATs
  (libretro-database). Covers and screenshots are downloaded per game from libretro-thumbnails into a
  local cache you can clear. Metadata can be edited per game.
- **Play features**: save states (10 slots with thumbnails; F1 save, F2 choose slot, F3 load), rewind, fast-forward, run-ahead (1–2 frames),
  Turbo A, CRT / LCD shaders (crt-easymode, crt-geom, crt-lottes, zfast-lcd), integer / aspect / stretch
  scaling, and a pause menu covering all of these.
- **Controls**: keyboard and gamepad remapping for all games or per game, rebindable hotkeys.
- **Resume**: quitting (and every minute of play) saves a resume point. The next Play offers **Continue** or
  **Start fresh**.
- **Soft patching**: `.ips` / `.ups` / `.bps` patches (and patch chains) placed beside a cartridge ROM are
  applied in memory. The ROM file is never changed.
- **Portable data folder**: the zip contains a `portable.txt` marker, so the library, saves, states,
  cores and settings all stay in `data\` next to `Nostyra.exe`. Settings → Data folder can move them.
- **RetroAchievements (softcore)** on 18 systems: log in once in Settings. You get an in-game overlay,
  unlock pop-ups with a chime, an achievements list in the pause menu and a launcher Achievements tab.
  - **Offline sessions**: a game you have played online once (same account) keeps its achievements
    with no network. Unlocks are queued and sent when you are back online.
  - **Patched-ROM fallback**: when RetroAchievements does not recognise a soft-patched game, Nostyra
    retries with the original ROM's achievements (softcore, on by default, can be turned off in Settings).

## Systems

| System | Core (downloaded on first play) | Status | BIOS | Achievements |
|--------|-------------------------------|--------|------|--------------|
| Super Nintendo (SNES) | Snes9x | **Stable** | Not needed | Yes |
| NES | Mesen | Experimental | Not needed | Yes |
| Game Boy / Game Boy Color | mGBA | Experimental | Not needed (optional `gb_bios.bin` / `gbc_bios.bin` in `data\system\`) | Yes |
| Game Boy Advance | mGBA | Experimental | Not needed (optional `gba_bios.bin` in `data\system\`) | Yes |
| Mega Drive / Genesis, Master System, Game Gear, SG-1000 | Genesis Plus GX | Experimental | Not needed | Yes |
| PlayStation | PCSX ReARMed | Experimental | Not needed (built-in HLE BIOS); optional own BIOS via Settings → BIOS for games that need it | Yes (not for `EBOOT.PBP`) |
| Nintendo 64 | Mupen64Plus-Next (OpenGL 3.3) | Experimental | Not needed | Yes |
| Sega Dreamcast | Flycast (OpenGL 3.3) | Experimental | Not needed (HLE); optional own `dc_boot.bin` via Settings → BIOS | Yes |
| Sony PSP | PPSSPP (OpenGL 3.3) | Experimental | Not needed (system files download with the core) | Yes |
| Nintendo DS | melonDS DS | Experimental | Not needed (built-in BIOS; **decrypted dumps only**) | Yes |
| Nintendo GameCube / Wii | Dolphin (OpenGL 3.3) | Experimental | Not needed (system files download with the core) | No |
| **Sony PlayStation 2** (new, desktop only) | PCEE2 — PCSX2 (OpenGL 3.3) | Experimental | **Required**: your own dump, asked for on the first Play (or Settings → BIOS) | Yes |
| **Atari 2600** (new) | Stella | Experimental | Not needed | Yes |
| **Atari 7800** (new) | ProSystem | Experimental | Not needed (an optional `7800 BIOS (U).rom` in `data\system\` is used if present) | Yes |
| **Atari Lynx** (new) | Handy | Experimental | Not needed (optional `lynxboot.img` in `data\system\`) | Yes |

- **Emulator cores are not bundled.** On the first Play of a system, Nostyra offers to download its core
  from the libretro buildbot (`buildbot.libretro.com`). You can also install cores in Settings →
  Emulator cores. Each core has its own licence. Snes9x and Genesis Plus GX are **non-commercial**; PCEE2 is GPL-3.0-or-later,
  Stella GPL-2.0-or-later, ProSystem GPL-2.0, Handy zlib.
- **No games or BIOS files are included or downloaded.** Use games you own and BIOS files dumped from your
  own console. PlayStation 2 is the first system that cannot start without one.

## Requirements

- Windows 10 or 11, 64-bit (x64).
- Microsoft Edge WebView2 Runtime. Windows 11 and updated Windows 10 include it. If it is missing, Windows
  shows an error with the download link (choose **Evergreen Bootstrapper**).
- A GPU with DirectX 12 or Vulkan support.
- Nintendo 64, Dreamcast, PSP, GameCube and Wii also need **OpenGL 3.3**. Without it the game stops with
  "This system needs OpenGL 3.3 graphics — update the graphics driver." Demanding Wii games may not run
  at full speed on slower CPUs (Dolphin runs single-core). PlayStation 2 uses OpenGL 3.3 too (without it the
  core falls back to a slower software path) and wants a fast CPU (about a 4-core Haswell or better).
- An internet connection only to download cores, artwork and achievement data. Library, metadata and
  play all work offline.

## Install

1. Download the zip from the official GitHub Releases page only.
2. Optional: right-click the zip → Properties → tick **Unblock** → OK.
3. Extract it anywhere you can write to (not `C:\Program Files`), then run `Nostyra.exe`.
4. The exe is not code-signed, so Windows SmartScreen may show **"Windows protected your PC"**. Click
   **More info**, then **Run anyway**.

## Known limitations

- **Windows only** in this release. macOS and Linux builds are not available yet (Nostyra Online runs in
  Chrome / Edge on any desktop OS).
- **Unsigned executable**: the SmartScreen warning above appears.
- **Experimental systems**: everything except SNES still needs manual testing on real games. Expect bugs.
- **RetroAchievements**:
  - Hardcore mode is hidden until RetroAchievements approves Nostyra as an emulator. All unlocks are softcore.
  - Leaderboards work only in hardcore mode, so this version has no leaderboard attempts or submissions.
  - Not available for PlayStation `EBOOT.PBP` games (use `.cue` / `.chd`) or for GameCube / Wii.
  - Chinese, Japanese and Korean characters show as boxes (□) in the in-game achievement overlay.
  - Copier `.smd` Mega Drive dumps are not recognised by RetroAchievements.
  - SNES SA-1 games: achievements that read the SA-1's own RAM may not work.
  - When offline, a game that has never been played online has no achievements. Its achievement set
    loads about 10–20 seconds after starting offline, and anything triggered before that is missed.
  - The patched-ROM fallback can misfire if the patch changes game memory.
  - Your login token is stored unencrypted (see Privacy).
- **Run-ahead and rewind** are off for Nintendo 64, Dreamcast, PSP, PlayStation 2, GameCube and Wii. Run-ahead on
  the DS is too heavy for most PCs.
- **Nintendo DS**: decrypted dumps only (No-Intro "Decrypted"). Your own BIOS / firmware and DSi mode are not
  supported. Loading a save state also restores the in-game save stored in it.
- **GameCube / Wii**: save states only load in the Dolphin version that made them. Discs and revisions of one
  game share a save folder. Choosing "Wii Menu" from the HOME menu hangs the game.
- **PlayStation 2**: Windows only so far (the core exists for Linux / macOS but is untested there). Native
  resolution only (no upscaling, widescreen or aspect options yet). Save states load only in the PCEE2 version and
  on the disc that made them; one black frame may show right after loading a state. Memory cards are shared by all
  games with no import / backup UI (copy the two files yourself). Every dump of one serial (`.iso`, `.chd`,
  revisions) is one library entry with one save folder. No pressure-sensitive buttons, rumble, multitap, light
  guns, cheats (`.pnach`), `.gz` / `.mdf` / `.nrg` images or homebrew `.elf`. Games that boot in digital pad mode
  ignore the sticks until they switch to analog themselves.
- **Atari**: a loose `.bin` is listed as Atari 2600 / 7800 only when the No-Intro database knows it (homebrew:
  rename to `.a26` / `.a78`); `.bin` inside ZIPs is not looked up. No paddles, driving controllers or keypads
  (D-pad instead). Stella and Handy show one stale frame right after a state load. A Lynx state loads only in the
  Handy version that made it; the Lynx EEPROM is written on Reset and on a normal exit (a crash loses that
  session's EEPROM writes). Atari 5200 / Jaguar / ST are not supported.
- **Cloud saves**: Google Drive and Local folder only in this release (OneDrive / Dropbox sign-in comes later);
  desktop only (Nostyra Online does not sync); not yet tested by hand across real PCs. Don't play the same game on
  two PCs at once: the conflict dialog catches it, but only after both have saved. A desktop sync app may need a
  moment to finish copying before the other PC sees the change.
- **Not supported yet**: Sega CD, 32X, PlayStation analog sticks / DualShock / rumble, PS1 `.img` / `.ccd` /
  `.mdf` / `.ecm`, core options, Big Picture / controller-only launcher mode, automatic updating (the launcher only tells you about a new release), Linux / macOS
  desktop builds.
- The launcher cannot capture keys that WebView2 keeps for itself (F10, Alt, the Windows key) for
  remapping. Edit `global.toml` to bind those.
- Logs and bug reports stay on your PC; nothing is uploaded automatically (see Reporting bugs).
- **Nostyra Online**:
  - Desktop Chrome or Edge only (Firefox, Safari and phones show an information page; Brave needs its File
    System Access flag).
  - PlayStation multi-disc games (`.m3u`) play disc 1 only; big PlayStation discs use a lot of memory (prefer CHD).
  - Don't play the same game in the browser and the desktop app at the same time (last save wins).
  - Closing the tab can lose up to 30 seconds of in-game progress; quit with **Exit game** instead.
  - A PlayStation controller is invisible to the browser while Steam Input (or DS4Windows / HidHide) holds it:
    exit Steam or turn off Steam Input for PlayStation controllers, reconnect the pad and press a button on the page.
  - Desktop 0.2.0 and older ignore play time recorded in the browser.
  - Lynx EEPROM saves are not synced with the desktop app (browser play keeps save states only).

The full list per system is in the README.

## Privacy

Nostyra has no telemetry and no accounts of its own.

**Stored on your PC** (in the portable copy: the `data\` folder beside `Nostyra.exe`): the library database,
saves, save states, settings, downloaded cores and cached artwork. If you log in to RetroAchievements:
- your username and login token, **unencrypted**, in `data\achievements\credentials.json`
  (your password is never stored);
- the last server answers (login, achievement sets, unlock lists) in
  `data\achievements\offline-cache\<user>\`, which also contains the token;
- unlocks waiting to be sent in `data\achievements\pending\`;
- achievement lists and badges in `data\achievements\games\` and `data\cache\achievement-badges\`.

Log out deletes the credentials and the offline cache. Anyone who can read your files can read the
token. It is not your password, and you can revoke it by changing your RetroAchievements password.

**Sent over the network**:
- **retroachievements.org** (only while logged in): your password once at login (to get the token),
  then your unlocks, rich presence and the game's RetroAchievements hash (an MD5 of the game data, never
  the file itself). Requests carry a user agent naming the Nostyra, core and rcheevos versions.
- **libretro-thumbnails** (`thumbnails.libretro.com` and the project's GitHub repositories): the game's
  name and system, to download its cover and screenshot.
- **libretro buildbot** (`buildbot.libretro.com`): which core (and its system files) to download.
- **GitHub** (`api.github.com`, update check): at most once a day at start and on **Check now**, one request for
  the latest Nostyra release with the user agent `Nostyra/<version>`; no identifiers, library or account data.
  Turn it off in Settings → Updates.

**Cloud saves**: there is no Nostyra server; your saves go only to your own storage.
- **Google Drive** (only while signed in): your browser signs in at `accounts.google.com`; Nostyra then talks to
  `oauth2.googleapis.com` and `www.googleapis.com` to upload / download the save files and small file lists of the
  games you play. It reads your Google account's display name and email once, to show which account is signed in.
  The sign-in is stored in `data\cloud\refresh-token.bin`, encrypted with Windows (DPAPI) for your Windows user, so
  a copied data folder on another PC or user asks you to sign in again. **Sign out** revokes it at Google and
  deletes the file. You can also remove Nostyra at https://myaccount.google.com/permissions.
- **Local folder**: Nostyra only copies files between your data folder and the folder you chose and contacts no
  server; your own sync app moves them.
- `data\cloud\` also holds the sync state (last-synced lists, backups, a random device id and your PC name, shown in
  the conflict dialog on your other PCs).

Set the environment variable `NOSTYRA_OFFLINE=1` to turn off all launcher downloads, the update check and the
background cloud sync at start.

**Nostyra Online** is a static website: your ROM files, saves and settings never leave your computer. The page
reads the folders you choose through the browser and keeps its own library database in browser storage. It only
downloads its own files from nostyra.com and covers / screenshots from libretro-thumbnails on GitHub
(`raw.githubusercontent.com`, `api.github.com`), sending the game's name and system.

## Reporting bugs

Open an issue at **https://github.com/vivusk/nostyra/issues**.

1. Reproduce the problem once, then close the game.
2. In the launcher, open **Settings → About** and click **Create bug report** in the **Logs** row. When a
   game fails, the error toast also has a **Create bug report** button.
3. Nostyra saves `nostyra-bug-report-YYYYMMDD-HHMMSS.zip` in `data\logs\`, opens that folder with the zip
   selected and shows **"Bug report saved — attach it to your GitHub issue"**.
4. Drag the zip into the issue and describe:
   - **Game**: the title as shown in the library and the file type (`.sfc`, `.cue` + `.bin`, `.chd`,
     `.rvz` …). Do not attach or link ROM, disc or BIOS files.
   - **Steps to reproduce**, what you expected and what happened. Mention the shader, run-ahead / rewind
     settings and any soft patches.
   - Screenshots or a short video for graphics, audio or input problems.

The zip holds the current and previous launcher logs, the 5 newest game logs and `system-info.txt` (Nostyra
version, Windows version, portable / default / custom data folder, installed cores and their versions), so
you do not need to copy versions by hand. Logs contain file paths, which may include your Windows user name;
open the zip and edit the logs first if you prefer. Your RetroAchievements password and token are never
logged, and any token-like value is scrubbed from the zip as a safety net.

**Open logs folder** (same row) opens `data\logs\`. Logs are cleaned up automatically: the newest 10
launcher logs and 30 game logs are kept, logs older than 14 days and bug reports older than 7 days are
deleted. For more detail, set `NOSTYRA_LOG=debug` (e.g. `set NOSTYRA_LOG=debug` in a Command Prompt, then
start `Nostyra.exe` from it) and reproduce the problem again.

## Licences

Nostyra is GPL-3.0-or-later. Nostyra Online lists its licences (EmulatorJS, each emulator core, the
libretro-database, npm and Rust components) at https://nostyra.com/play/licences.html. The `licenses\` folder in the zip has the licence texts for the bundled
third-party code and data: Rust crates (`THIRD-PARTY-RUST.txt`), native libraries and other bundled files
(`THIRD-PARTY-NATIVE.txt`: SDL3, glslang, SPIRV-Cross, SQLite, the Visual C++ runtime), the launcher's npm
packages (`THIRD-PARTY-NPM.txt`), rcheevos, the Noto Sans fonts, libretro-database (CC-BY-SA-4.0) and the
slang shaders.
