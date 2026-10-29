# Nostyra — System Architecture

Last updated: 2026-10-29 · Status: M0–M4 + M6–M15 implemented (Windows; manual checks pending)

## 1. High-Level View

```
┌──────────────── nostyra (launcher, Tauri 2) ────────────────┐      ┌──────────── nostyra-runner (per game) ────────────┐
│ React UI: Home · Library · Detail · Settings (Controls)     │      │ winit window + wgpu renderer (+ librashader)      │
│        │ tauri::command / events                            │ IPC  │ libretro-host ──dlopen──► <core>_libretro.dll/.so │
│ Rust backend: game-library · artwork-cache · storage-db     │◄────►│ audio-output (SDL3 + dynamic rate control)        │
│               config-layers · core-manager · runner-spawner │ JSON │ input-mapping (SDL3 gamepad + keyboard)           │
└─────────────────────────────────────────────────────────────┘ lines│ overlay layer: OSD + pause menu         [M3]      │
                                                                     └───────────────────────────────────────────────────┘
```

**Why two processes**
- Core crash (C code) cannot take down the launcher.
- Emulation loop is isolated from webview scheduling → stable latency.
- Runner is usable standalone: `nostyra-runner --core <path> --rom <path> [--system <id>]` (shortcuts, Steam "Non-Steam game"); without `--system` the ROM is classified like the scanner does (§5.1).

## 2. Repository Layout (Cargo workspace)

```
nostyra/
├── apps/
│   ├── launcher/              # Tauri 2: src-tauri/ (Rust) + src/ (React)       [M1]
│   └── runner/                # bin: nostyra-runner                             [M0]
├── crates/
│   ├── libretro-host/         # libretro FFI, core loading, callbacks           [M0]
│   ├── video-renderer/        # wgpu frame upload, scaling, shaders             [M0/M3]
│   ├── hw-render-gl/          # OpenGL backend of the hw-render API: hidden SDL3 GL context, FBO, readback [M10]
│   ├── audio-output/          # SDL3 audio stream, resampler, DRC               [M0]
│   ├── input-mapping/         # SDL3 devices → RETRO_DEVICE_JOYPAD / ANALOG (sticks, triggers), pointer state (DS touch, Wii IR), config bindings [M0/M4/M10/M11/M13/M14]
│   ├── ipc-protocol/          # launcher↔runner message types (serde)          [M1]
│   ├── storage-db/            # SQLite schema, migrations, repositories        [M1]
│   ├── game-library/          # scanner, hashing (streamed > 32 MiB), DAT matching, DS game code, GC/Wii game-ID identity (no disc hashing) [M1/M13/M14]
│   ├── rom-patch/             # IPS/UPS/BPS soft-patching (in memory)           [M2]
│   ├── core-manager/          # buildbot core download + atomic install; core system files (all or nothing) [M1/M12]
│   ├── runner-session/        # Tauri-free runner spawn/events/play-time import [M1]
│   ├── http-download-util/    # shared HTTPS client (offline switch), capped copy [M2]
│   ├── artwork-cache/         # libretro-thumbnails index, download, re-encode   [M2]
│   ├── config-layers/         # TOML layers global→system→core→game, merge, write; binding names; staged dir replace [M3/M4/M12]
│   ├── system-catalog/        # std-only registry: systems, cores, ROM classification, header rules, BIOS status, touch-screen / aiming-pointer rule [M6, M8, M13, M14]
│   ├── disc-image/            # cue/gdi/m3u, CHD (CD + GD-ROM + DVD), CDI, CSO, sector reader, ISO9660 paths, PARAM.SFO, PS1 id + serial, DC IP.BIN, PSP disc id, GC/Wii disc headers (ISO/GCM/CISO/GCZ/WIA/RVZ/WBFS) + DOL/ELF HID4 scan, serial rules, DiscImageProbe, track list + absolute-LBA reads [M8/M11/M12/M14/M15]
│   └── achievements/          # rcheevos FFI (private `ffi`, the only unsafe), safe single-thread rc_client wrapper, transports (ureq | fake), credentials, offline journal, hashing readers, memory regions, hardcore rules, snapshot schema [M15]
├── assets/fonts/              # Noto Sans Regular + Bold (OFL-1.1), embedded in the runner for the achievement overlay [M15]
├── third-party/libretro/      # vendored libretro.h (MIT)
├── third-party/rcheevos/      # vendored rcheevos v12.5.0 include/ + src/ (MIT, unmodified), built by crates/achievements/build.rs (cc) [M15]
├── third-party/libretro-database/ # No-Intro DATs (SNES [M1], NES [M6], GB/GBC/GBA [M7], Sega [M9], N64 [M10], NDS Decrypted [M13]), Redump PS1 [M8] + Dreamcast [M11] + PSP [M12] + GameCube / Wii [M14] + metadat attribute DATs (developer, publisher, genre, releaseyear, maxusers) [M2], CC-BY-SA-4.0
├── docs/                    # plans/ is local-only (git-ignored)
```

## 3. Runner Internals

### 3.1 Main loop (single emulation thread)
```
loop {
  pump window + SDL events  → update input state
  core.retro_run()          → callbacks fill: frame buffer, audio samples, input polls
  renderer.present(frame)   → wgpu, FIFO (vsync mode) or mailbox/immediate (audio-sync mode)
  audio.push(samples)       → resample with ratio adjusted by buffer fill (DRC)
}
```
- Sync mode auto-selected from display refresh (winit `refresh_rate_millihertz`): vsync base ratio `base = (display_hz / core_fps) × (core_rate / round(core_rate))` (mode selection + vsync DRC only).
  - `|base − 1| ≤ 0.01` → **vsync mode**: timing master = display vsync (Fifo); DRC `ratio = base × (1 + d·(fill − target)/target)`, clamped ±d (d ≈ 0.5%), keeps buffer at target (~64 ms).
  - Otherwise (e.g. 59/61/120/144 Hz, or refresh unknown) → **audio-sync mode**: emulation at core fps, block until queued ≤ target; audio ratio = `core_rate / round(core_rate)` only (no display factor; optional DRC around it). Present once per emulated frame: Immediate → Mailbox → Fifo (warn if display_hz ≤ core_fps; audio wait throttles). `--audio-sync` forces it.

- Run-ahead (M3, `latency.run_ahead_frames` 0–2, single instance): per due frame the real frame runs (video off, audio on) → `serialize` (`RUNAHEAD_SAME_INSTANCE` context, reused buffer) → N−1 hidden frames (AV off) → 1 frame video on / audio off → present → `unserialize`. Catch-up ticks run plain frames for all but the last due frame. SRAM tick, audio pacing and rewind capture see only real-timeline frames. Bypassed while fast-forward, rewind or the pause menu is active; refused for cores with the `INCOMPLETE` serialization quirk and switched off for the session on a serialize error (OSD "Run-ahead unavailable"). Debug log every 10 s: avg/max emulate time per tick (`emulate_time_stats`).
- Serialize plumbing (`libretro-host`): `serialize_into` / `unserialize_from` reuse one buffer, cache `serialize_size` (re-queried for `CORE_VARIABLE_SIZE`), honour `SET_SERIALIZATION_QUIRKS` (`MUST_INITIALIZE` → `SerializeNotReady`), answer `GET_SAVESTATE_CONTEXT` (`NORMAL` for slots, `RUNAHEAD_SAME_INSTANCE` for run-ahead/rewind); `AvEnable{video,audio}` replaces the single skip flag.
- Achievement hooks (M15, only while an achievement session runs, §3.7): `rc_client_do_frame` after every real-timeline frame (`RealTimeline::after_real_frame`) and every fast-forward frame (skipped + preview); run-ahead's hidden frames never reach it (rollback = the real state, so no achievement progress in run-ahead snapshots). `rc_client_idle` on every paused and rewinding redraw; `rc_client_reset` from the menu reset; disc swap → own hash → `begin_change_media`. No session (logged out, disabled, unsupported system) = no hook cost.
- Soft-patching (runner start): `rom-patch::find_patches_for_rom` builds the chain beside the ROM — `<rom stem>.bps`, `.bps1`, `.bps2`, … then `.ups…`, then `.ips…` (unnumbered file optional, numbering stops at the first gap) — and `apply_patch_chain` applies it in order (any failure → unpatched, never half-patched); the patched bytes go to the core via `SessionConfig::rom_data` (cores needing the full path ignore it, with a warning). UPS/BPS verify CRC32s (retried on the headerless image for 512-byte copier headers); a failing chain is logged and the game starts unpatched. Saves stay keyed by ROM name (shared with the unpatched game).

### 3.2 libretro callback context
libretro callbacks are plain C fns without user-data pointers. `libretro-host` stores the active session in a thread-local on the emulation thread (`thread_local! { static CTX: Cell<*mut HostContext> }`), set for the whole session lifetime (load → unload), since env callbacks also fire during `retro_set_environment`/`retro_load_game`. No Mutex (callbacks run inside `retro_run` → deadlock risk). `CoreSession` is `!Send`. One core instance per process — matches the runner model; enforced by a process-wide flag (second load → `SessionAlreadyActive`). Hence the **single-thread rule** for new cores: input and audio callbacks answered off the session thread see no context (input 0, audio dropped), so a core with its own emulation threads runs in its single-thread mode — Dolphin (M14) with the fixed option `dolphin_main_cpu_thread=disabled` (CPU + GPU emulation inside `retro_run`; `serialize` with the GL context current). PPSSPP's render thread only needs the process-wide FBO name (§3.3). Dual core = backlog (process-wide input snapshot + thread-safe audio sink).
Memory maps (M15, `environment_memory_maps.rs`): env 36 `SET_MEMORY_MAPS` deep-copies the descriptors and their `addrspace` strings into `HostContext` (`MemoryMapSnapshot`, `generation()` bumped per call; a core may free its array right after the call); env 42 `SET_SUPPORT_ACHIEVEMENTS` is accepted and only logged (only Snes9x and melonDS DS send it). `CoreSession::{memory_maps, memory_info(id)}` expose them to the achievement session.
Core logging: `retro_log_printf_t` is variadic, so a tiny C shim (`vsnprintf`) formats messages and forwards them to `tracing` (target `core`).
Call order: `retro_set_environment` → `retro_init` → `retro_set_video_refresh/audio/input` → `retro_load_game` → `retro_get_system_av_info` (→ hardware context + `context_reset` for GPU cores, §3.3).

### 3.3 Video
- Software frames: `RGB565` (Snes9x default), `XRGB8888`, `0RGB1555` → converted to RGBA8 on CPU (SNES ≤ 512×478, NES 256×240, GB/GBC 160×144 (256×224 with a Super Game Boy border), GBA 240×160: tiny frames).
- **Hardware rendering (M10)** — cores that draw on the GPU (`SET_HW_RENDER`; Mupen64Plus-Next / GLideN64, Flycast since M11, PPSSPP since M12):
  - *API* (`libretro-host::hw_render`, backend-agnostic, no GL/Vulkan types): `HwContextKind` (`OpenGl` compat, `OpenGlCore{major,minor}`, GLES, `Vulkan`, `Other`), `HwRenderRequest` (kind, depth, stencil, `bottom_left_origin`, …), trait `HwRenderProvider { supports, preferred, create(request, max_w, max_h) }` and trait `HwRenderContext { make_current, framebuffer_id, proc_address, deliver_frame(w, h, &mut dyn VideoSink) }`. `SessionConfig.hw_render: Option<&dyn HwRenderProvider>` only has to live during `load`; the session (`HostContext`) owns the created context.
  - *Environment* (`environment_hw_render.rs`): `SET_HW_RENDER` → `supports`? store the request and fill `get_current_framebuffer` / `get_proc_address` : refuse (the core's `retro_load_game` fails → `HostError::HwRenderUnavailable` → exit 6); `GET_PREFERRED_HW_RENDER` → `preferred`. `video_refresh` with `RETRO_HW_FRAME_BUFFER_VALID` (`(void*)-1`) only marks a frame pending.
  - *Call order* (`core_session_hw_render.rs`, RetroArch's): `retro_load_game` (SET_HW_RENDER) → `retro_get_system_av_info` → `create` at the av **maximum** size → make current → `context_reset` → per frame: make current before **every** core entry (`retro_run`, `serialize`, `unserialize`, `reset`) → after `retro_run` `deliver_frame` of the pending frame → teardown: make current → `context_destroy` (only if `context_reset` ran) → `retro_unload_game` → `retro_deinit` → drop the context → unload the library. A core that asked for a context but never got `context_reset` (failed create) skips `retro_deinit` (GLideN64 crashes there through unresolved GL symbols; the runner exits anyway). FFI calls never hold a `&mut HostContext` (the core re-enters `get_current_framebuffer` from `retro_run` / `retro_serialize`).
  - *OpenGL backend* (`crates/hw-render-gl`, `unsafe` allowed): `SdlGlProvider` serves `OpenGl` with a **compatibility-profile** context (Mupen asks for type `OPENGL`, not `OPENGL_CORE`) and `OpenGlCore ≤ 3.3` with a core one; SDL (video) is initialised lazily in `create`, so software cores never start SDL video. `GlOffscreenContext` = hidden 1×1 SDL3 window (never shown or swapped), swap interval 0, one FBO for the session (RGBA8 texture + `DEPTH24_STENCIL8` renderbuffer, stable id: cores cache it), one pixel-pack buffer. `deliver_frame` = synchronous `glReadPixels` → PBO → map → `VideoFrame { Rgba8888, bottom_up }`, then restores the PBO / read-framebuffer / pack-alignment bindings it touched (a bound PBO would redirect GLideN64's own RDRAM copies). Sync readback costs p95 0.91 ms at 960×720 (spike), so no async ring. `probe_gl_context()` tells tests whether a GL 3.3 context can be made.
  - *Presentation*: the readback lands in the usual `RgbaFrameBuffer` (converter flips `bottom_up` rows, forces alpha 0xFF), so the wgpu presenter, shaders, overlay, thumbnails and headless runs are unchanged. While a GL session is active wgpu never uses its GL backend (`WGPU_BACKEND=gl` → warning, `PRIMARY`); on Windows DX12 presents beside the hidden GL window.
  - *Runner*: one `HwRenderSetup` (wraps `SdlGlProvider`) per run, built before `CoreSession::load` (windowed, headless and `--print-config`) and dropped after the session. Debug builds read `NOSTYRA_FORCE_NO_GL=1` to make `create` fail (process tests for exit 6); release builds ignore it. Window: 3× base height, or 720 logical px when that exceeds 1080 (N64 base 960×720 → 720), capped at 90 % of the monitor.
  - *Framebuffer growth (M11)*: the FBO is allocated at the av maximum (Flycast: square, 1706×1706 at 1280×960). A `SET_SYSTEM_AV_INFO` whose maximum exceeds it calls the required `HwRenderContext::resize` inside the core call (`environment_av_info.rs`; the context is current): `gl_target_resize.rs` swaps the immutable colour texture, re-stores the depth/stencil renderbuffer and the PBO, keeps the **FBO id** (cores cache it) and restores every binding it touched. `SET_GEOMETRY` never resizes; frames are read back at the `video_cb` size. `HwRenderContext::size` is required too. Flycast asks `OPENGL_CORE 3.2` (served by the 3.3 core context; a compat answer would also work).
  - *Core shutdown (M11)*: `RETRO_ENVIRONMENT_SHUTDOWN` is accepted; the next `run_frame` returns `HostError::CoreShutdown(<last core message>)` (Flycast after a fatal emulator error, e.g. "Failed to locate bootfile"), so the runner ends with exit 5 and the reason instead of showing a frozen frame.
  - *PPSSPP (M12)*: asks `OPENGL_CORE 3.1` (served by the 3.3 core context) and renders from its own emulator thread: `get_current_framebuffer` called off the session thread answers a process-wide FBO name (`publish_session_framebuffer`; one session per process), else PPSSPP would draw into the hidden window. A refused `SET_HW_RENDER` that the core survives in software is logged as a warning (`runner_session_config.rs`); `ppsspp_backend=none` (software) is a test-only `--core-option`, never a user fallback (no GL → exit 6 as for N64 / DC). After a failed context creation the core is **kept loaded** (`CoreLibrary::keep_loaded`: no `retro_unload_game` / `retro_deinit`, library leaked) and the runner reports exit 6, then hard-exits (`process_hard_exit.rs`: `TerminateProcess` / `_exit`), because a normal exit with such a core loaded crashes. PPSSPP sends no shutdown message, so `HostError::CoreShutdown` carries the core's last error-level log line (`environment_core_messages.rs`, process-wide: cores log from their own threads), e.g. "[Boot] Failed to load executable: …".
  - *Dolphin (M14)*: asks `OPENGL_CORE 3.3` (= `MAX_CORE`, no host GL change; the driver context stays exactly 3.3, so no compute / SSBO → no bounding box / GPU texture decode, documented limit), renders on the session thread (single core, §3.2), EFB 1× (`dolphin_efb_scale=1`, 640×528; spike readback p95 1× 2.89 ms vs 2× 7.01 ms). New `CoreSpec` flags (default false, all three true only for `DOLPHIN`):
    - `hw_render_required`: Dolphin survives a refused `SET_HW_RENDER` and silently runs its Null backend (black picture). `runner_session_config::fatal_hw_refusal(required, refused, has_context)` turns "loaded, no context, a request was refused" into `HostError::HwRenderUnavailable` with a generic text ("this core needs OpenGL 3.3 hardware rendering…", never the refused kind: the last refused request is `Vulkan`) → `CoreSession::abandon_core` (no `retro_unload_game` / `retro_deinit`, library kept) → exit 6 + hard exit. Unflagged cores keep the PPSSPP warning. `dolphin_renderer=Null` (tests only) sends no request, so it is never refused. Debug hook `NOSTYRA_FORCE_NO_GL=refuse` makes `supports()` false (process test). The flag also gates states (`core_session_core_rules.rs`): a flagged session without a hw context has `states_available() == false`; `serialize_into` / `unserialize_from` → `HostError::StatesNeedHwContext` and `current_state_size` → 0 **without any core call** (Dolphin segfaults in `retro_serialize_size` under Null / Software). The runner refuses cleanly before that: startup resume / `--load-state` (`StartupTarget::states_available`), F4 / menu load, F1 / menu / headless save ("Save states unavailable without hardware rendering"), exit auto-save skipped; run-ahead / rewind are already refused (`Serialized`).
    - `keep_library_loaded`: `FreeLibrary` after any `retro_unserialize` (and after a failed boot) hangs the session thread's exit, so `SessionConfig::keep_library_loaded` keeps the library mapped from the start of `CoreSession::start` (unload + deinit still run). Its DLL-detach / static-destructor phase can also hang a normal process exit, so `main` ends every session of such a core with `process_hard_exit::exit_now` (Windows `TerminateProcess`, Unix `_exit`) after the unload, save writes, IPC `Stopped` and the pending play-session record; `loaded_session_run` flags it just before `CoreSession::load`, so a failed load counts too.
    - `strict_state_version`: see the state guard (§5.1, GameCube / Wii).
  - *Teardown watchdog (M14, all cores)*: `process_hard_exit/teardown_watchdog.rs` (open core issue #520: unload hang). `TeardownGuard` arms on drop and is declared after the value owning the session on each path (after `CoreSession::load` with `EXIT_RUNTIME` for `?` early returns; re-bound after `app.finish()` (now `&mut self`) on the window path and to 0 at the end of `headless_runner::run`); `main` disarms before its own `reporter.finished`. After `TEARDOWN_TIMEOUT` (10 s; debug `NOSTYRA_TEST_TEARDOWN_TIMEOUT_MS` / `_DELAY_MS`) it logs a warning, reports `finished(code, "core teardown timed out")` through the shared `SessionReporter` (`Arc`, started state read at fire time, an atomic "reported" flag so only one side reports: IPC `Stopped` + pending play-session record) and calls `exit_now(code)`; code = the run's own (0 on a normal quit). Not armed during a failed load (keep-loaded covers Dolphin there).
  - *Next backends*: another GL core needs no host change beyond what it asks for (`OpenGlCore` above 3.3 → raise `MAX_CORE` after testing; Dolphin's bounding box would need it). Vulkan (ParaLLEl-RDP, Dolphin) = a new `HwRenderProvider` crate plus the `SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE` / `GET_HW_RENDER_INTERFACE` env ids; a zero-copy backend hands a GPU texture to a GPU-side sink through the same `deliver_frame` seam instead of reading back. Context loss / re-init is not handled (we never re-init video).
- **Two screens (M13, melonDS DS)**: software `XRGB8888`, both DS screens composited by the core into one frame (top-bottom 256×384 → window 480×720 by the start-up rule). R3 switches the core's layout (`SET_GEOMETRY` → 512×192 left-right and back). On a base-geometry aspect change the runner resizes a normal window to the start-up size for the new shape (`emulation_main_loop/window_resize_on_geometry.rs`: `resize_target` pure, `apply_geometry` from the av-change hook; `runner_window::window_size_for` shared with start-up); fullscreen / maximised windows, an unchanged aspect (relative < 1e-3) or an unknown previous aspect only re-fit the picture. Generic for every core (others keep one aspect). `SET_ROTATION` stays unhandled (rotated layouts are not offered).
- Upload to wgpu texture, draw fullscreen quad; scaling modes: aspect-correct (8:7 PAR → 4:3), integer scale, stretch.
- Frame texture stored as `Rgba8Unorm`; the plain quad samples an `Rgba8UnormSrgb` view.
- Shader stage (M3): `ShaderStage::{Plain, Chain}`. `video.shader` (`none` | bundled preset | absolute `.slangp`) → librashader `runtime-wgpu` filter chain built on a worker thread (newest request wins); the old chain / plain quad keeps rendering until `poll_shader_load` swaps it in. Build or wgpu validation error → plain quad + OSD "Shader failed". The chain renders through a non-sRGB view of the surface (RetroArch-style UNORM back buffer). The device requests only the optional features the adapter offers. librashader's pipeline cache needs `PIPELINE_CACHE` (Vulkan only), so it is effectively off on DX12/Metal.
- Bundled presets (`config_layers::BUNDLED_PRESETS`, file paths from `bundled_preset_rel_path`, upstream layout): `crt/` (crt-easymode, crt-geom, crt-lottes) and `handheld/` (zfast-lcd, M7).
- Bundled presets lookup (runner): `--shaders-dir` (launcher passes `<resource_dir>/shaders`) → `NOSTYRA_SHADERS` → `shaders/` beside the exe → repo `third-party/slang-shaders` (debug builds only).
- Overlay layer (M3): one CPU-rasterised RGBA canvas at output resolution (5×7 font ×1/×2/×3 by output height, thumbnails blitted CPU-side), uploaded only when it changes and alpha-blended **after** the shader pass, so OSD text and the pause menu are never CRT-filtered.
- Achievement overlay (M15): per redraw one canvas = pause menu (with the Achievements page) **or** the composed overlay (`achievement_overlay/overlay_composer.rs`: status line + popups + trackers / indicators), repainted only when the output size, picture rect, status text or a layer revision changes (`None` when nothing shows). Unicode text through `video-renderer` `TtfText` (fontdue 0.9.4, embedded Noto Sans Regular / Bold parsed once per process on first need, glyph cache keyed (face, px, char) bounded 4096, NFC before layout, `.notdef` box for missing glyphs, ellipsis / wrap) blended with `OverlayCanvas::blend_mask`; badges 64×64 RGBA blitted nearest-neighbour. Sizes scale with output height (popup text h/40, list h/45).

### 3.4 Audio
- Core output: int16 stereo at the core's rate (Snes9x ~32040.5 Hz fractional, Mesen 48 000 Hz, mGBA 131 072 Hz GB/GBC and 65 536 Hz GBA, Genesis Plus GX 44 100 Hz at 59.92 fps, 735/736 frames per video frame; Mupen64Plus-Next 44 100 Hz at 60 fps NTSC). `audio_sample_batch` reports frames consumed even with no sink installed (Mupen pushes audio while `retro_serialize` runs to a safe point and loops until accepted). Device: 48 kHz typical.
- Unlock chime (M15, `audio-output::SoundEffectStream`): 300 ms A5→E6 sine synthesized at 48 kHz mono, fixed gain, on its **own** `SDL_OpenAudioDeviceStream` on the default device (binding a second stream to the runner's device fails; SDL mixes the two logical devices); muted with `achievements.sound = false` and while fast-forwarding.
- SDL3 `AudioStream` opened at `round(core_rate)` resamples to device; ratio set per frame via `SDL_SetAudioStreamFrequencyRatio` = mode base × DRC (vsync: display-factor base; audio-sync: `core_rate/round(core_rate)`; see §3.1). Device paused until first prefill reaches target. No custom resampler unless quality/latency insufficient.

### 3.5 Input
- SDL3 gamepad (uses SDL controller DB; Steam Input passthrough on Deck).
- Bindings (M4): one logical layer, action → 0–2 physical inputs per device, read from config at start (§3.6 sections `input_keyboard`, `input_gamepad`, `hotkeys_keyboard`, `hotkeys_gamepad`). Bound by **position**: keyboard names = winit `KeyCode` variant names (physical scancode), pad names = own snake_case positional names (`south east west north back guide start left_stick … left_trigger right_trigger`), shown with Xbox or Sony labels. `config-layers::input_binding_names` owns every name, the defaults (M3 layout: arrows, Z/X/A/S = B/A/Y/X, Q/W = L/R, Enter = Start, Right Shift = Select; pad south/east/west/north = B/A/Y/X, shoulders, Start, Back = Select, D-pad) and the pad label table; `input-mapping` converts names (`key_names`, `gamepad_input_names`) and builds `InputBindings` (key/pad → joypad mask, key/pad → hotkey; `keyboard_state` keeps the held-key set). Keyboard drives port 1 only; `input_gamepad` applies to every pad. Triggers count past half travel (usable as game buttons or hold hotkeys); left stick → D-pad (`stick_dpad`) for systems without analog sticks.
- Analog sticks (M10; `SystemSpec.analog_sticks: AnalogSticks` since M13 — `None` (default), `Both` = N64, Dreamcast, PSP, `RightOnly` = NDS; one enum in `system-catalog`, re-exported by `input-mapping`, `AnalogInputs.sticks`): SDL sticks go **raw** to `RETRO_DEVICE_ANALOG` (index 0 left, 1 right; `-32768` clamped to `-32767`; +Y down as in SDL and libretro) and no host deadzone is applied (the core's own, Mupen 15 % radial). `input_mapping::analog_stick_mode`: `left_stick_dpad` (left stick → D-pad unless `Both`) and `raw_stick_axes` (`RightOnly` → `[0, 0, rx, ry]`: the DS keeps its D-pad on the left stick and gets only the right stick raw, for the core's touch cursor). `PAD_ACTIONS` gained 8 stick actions in M10 and `l3` / `r3` (stick clicks, RetroPad ids 14 / 15, `JoypadButton::{L3, R3}` in `joypad_button.rs`) in M13 → 24 total, in libretro id order: the 16 buttons (`… l2 r2 l3 r3`), then `lstick_up/down/left/right`, `rstick_up/down/left/right`; `l3` / `r3` default to the pad's `left_stick` / `right_stick`, keyboard unbound; keyboard defaults I/K/J/L and T/G/F/H, pad defaults unbound (the pad's sticks already feed the axes). A held key or button gives full deflection and wins over the axis; opposite directions cancel (`input_mapping::analog_stick_input`; action bits: buttons low 16, sticks above, `game_action_bits`). Systems without analog sticks list no stick actions in `pad_actions`, so Controls hides them (SNES still 12 rows) and their analog reply is 0. N64: right stick = C-buttons (core option, threshold 0x4000), Z on `l2` = left trigger, so the hold-hotkey yield rule takes LT from rewind; RT keeps fast-forward.
- Analog triggers (M11, `SystemSpec.analog_triggers`, Dreamcast = true; `SdlGamepadManager::new` takes `AnalogInputs { sticks, triggers }`): `RETRO_DEVICE_ANALOG` index 2 (`RETRO_DEVICE_INDEX_ANALOG_BUTTON`) ids L2 / R2 answer the pressure of the pad inputs bound to `l2` / `r2` (`input_mapping::analog_trigger_input`: a trigger gives its travel 0..0x7FFF, a button full travel; a rebind keeps working), and full travel when only the digital bit is held (keys); the digital L2 / R2 bits stay (cores fall back to them). A trigger held when the pause menu closes reads 0 until released. Other systems answer 0 on index 2, as before. Dreamcast pad: `b a y x start` + D-pad + `l2 r2` + left stick (labels A / B / X / Y / LT / RT); both triggers are game buttons, so the hold hotkeys yield on the pad (keyboard keeps them).
- Pointer / touch screen (M13): `RETRO_DEVICE_POINTER` (6, ids X / Y / PRESSED / COUNT / IS_OFFSCREEN) is answered on port 0 only for systems with `SystemSpec.touch_screen: Option<TouchScreenRule>` (others → 0, as for every unknown device). `input_mapping::PointerState { x, y, pressed, offscreen }` (`reply(index, id)`; `fraction_to_pointer` maps 0..1 to −0x7FFF..0x7FFF). Runner `emulation_main_loop/pointer_events.rs`: window events only store the cursor (physical px) and the left button (`CursorMoved`, `MouseInput(Left)`; `CursorLeft`, `Focused(false)` and opening the pause menu release; presses while the menu is open are ignored); once per redraw `update_pointer` maps the cursor over `game_viewport()` (the whole composited frame, which the core expects) with the current `av_info().geometry`, so resizes and layout swaps apply without a mouse move. **Pressed only inside the touch rect**: `touch_rect(rule, w, h)` in frame fractions (`DsBottomScreen`: 2:3 → bottom half, 8:3 → right half, else the whole frame), so a press on the DS top screen is not a touch (the core would clamp it to the bottom edge) and dragging out of the rect releases. The OS cursor stays visible. Pad touch is the core's own joystick cursor (right stick + R2, `touch_mode=auto`: last used of pointer / stick wins); the cursor position is core frontend state, not in the savestate (after a load it stays, under run-ahead N it moves N+1× per frame).
- Wii Remote IR (M14): `TouchScreenRule::WiiRemoteIr` on `WII.touch_screen` (field name kept; `TouchScreenRule::aims()` = true) → the same `MousePointer` + port-0 gating; `touch_rect` = the whole frame. Dolphin `dolphin_ir_mode=2` reads `RETRO_DEVICE_POINTER` X / Y (index 0) on the Wiimote's port (P1 → 0) only; PRESSED / COUNT / IS_OFFSCREEN are never read. Aiming rules (`emulation_main_loop/pointer_aim.rs`): cursor over the picture → X / Y over the whole frame; off the picture or out of the window → `offscreen` and x / y = the cursor clamped to the nearest picture edge (no snap to the centre: Dolphin ignores `IS_OFFSCREEN`, so the IR parks at the edge); the OS cursor is hidden only while aiming over the picture with the pause menu closed (the game draws its own pointer; the default `ir_yaw/pitch` make the two drift apart), shown again by `release_pointer` (menu open, focus loss, cursor left). Mouse buttons map to nothing (A / B stay on pad / keyboard). Player 2 has no IR; pad-only players cannot aim (right stick = Remote tilt in mode 2). DS touch behaviour is unchanged.
- GameCube / Wii pads (M14, `port_devices` `[JOYPAD, JOYPAD, NONE, NONE]`, 2 players = `PORT_COUNT`): GC = A / B / X / Y, Start, D-pad, `r` = Z, `l2` / `r2` = analog L / R (`analog_triggers`, index 2), `l3` / `r3` = their digital light press, left stick + C-stick (`AnalogSticks::Both`). Wii (`dolphin_wiimote_default_device=nunchuk` turns the same JOYPAD into Remote + Nunchuk): A / B, Nunchuk C / Z = `x` / `y`, 1 / 2 = Start / Select, − / + = L / R, shakes = `l2` / `r2` (digital), Home = `r3`, Nunchuk stick = left, Remote tilt = right. Both systems list `l2` + `r2`, so the pad's LT / RT hold hotkeys yield (`input_conflicts`); `dolphin_hotkey_sideways_toggle` is off (L3 is a game input). Labels come from `pad_labels` (Controls shows `A` rather than `A (A)` when the names match).
- NDS pad (M13): `b a y x select start` + D-pad + `l r` + `r2` ("Touch (stick cursor)") + `l3` ("Microphone (blow)", `mic_input=blow`, hold) + `r3` ("Swap screens") + `rstick_*` ("Cursor …"). R2 is a game button, so the pad's RT fast-forward hold yields (`input_conflicts`, silent); LT rewind and the keyboard hotkeys stay. Lid not exposed.
- PSP pad (M12): `b a y x select start` + D-pad + `l r` + left stick (labels ✕ / ○ / □ / △ / L / R / Stick …; PPSSPP's own 0.15 deadzone), no analog triggers; L2 / R2 stay the fast-forward / rewind hold hotkeys.
- Controller port devices (M11, `CoreSpec.port_devices`): the runner calls `retro_set_controller_port_device` for each listed port right after `CoreSession::load` (`CoreSession::set_controller_port_devices`); empty list = no call (existing cores unchanged). Flycast and Dolphin `[JOYPAD, JOYPAD, NONE, NONE]`: without the four calls it connects no pad and no VMU; a `retro_reset` keeps them.
- Fixed inputs (never bindable, rejected by validation and by the launcher): **Esc** (opens / Back in the pause menu, cancels launcher capture), pad **Guide** (opens / Back), and the Select+Start 0.5 s chord on the physical `back` + `start` buttons — so `back` / `start` are refused in `hotkeys_gamepad` (still bindable as game actions). Menu navigation (arrows / Enter / Esc / Backspace; D-pad / south / east) and window close / Alt+F4 are fixed too.
- Conflict rules: per-key validation (unknown name, a fixed input, > 2 entries, wrong type, wrong device) drops that key with a warning naming the layer. After the merge one owner, `config-layers::resolve_input_conflicts` (used by runner and launcher), applies: input on a hotkey and a game action → **hotkey wins** (removed from the action, warning); input on two hotkeys → first in `HOTKEYS` order (`pause_menu save_state load_state prev_slot next_slot toggle_fullscreen fast_forward_hold rewind_hold turbo_a_toggle`) wins; one input on two game actions is allowed (presses both). The launcher never writes a conflict: `assign_binding` steals the input from its previous holder in the same write.
- Pause menu (M3): Esc, the `pause_menu` binding (default F10; any key or pad input bound to it also acts as Back while the menu is open, after fixed navigation), pad Guide, or Select+Start held 0.5 s (both buttons reach the core during the hold). While open: no `retro_run` (no rewind capture / run-ahead / SRAM tick), queued audio dropped, game input released, last frame presented with its shader under a 60 % dim + panel on the overlay layer (5×7 font, ×1/×2/×3 by output height). Menu = pure state machine (`pause_menu/`) emitting actions: resume, save / load slot (2×5 grid, thumbnails decoded once per open, a menu save updates its cell from memory, overwrite confirm), live options (shader async, scaling, run-ahead, rewind; session-only until "Save for this game" / "Save as default" writes the keys that differ from the resolved config to the game / global layer, "Reset game settings" removes the menu option keys (`MENU_OPTION_KEYS`) from the game layer), reset game (M4: Yes / No question, default No; Yes = SRAM flush → `retro_reset` while paused → rewind history cleared → menu closes, OSD "Game reset"; SRAM and resume point untouched), quit (Yes / No question, default No, Back = No; Yes = window close path). Input goes to the menu before game bindings; keys / pad buttons still held on resume are swallowed / hidden from the core until released; menu repeat 400 ms then 80 ms. Close = pacer reset + audio re-prefill. Play time keeps counting while paused.
- Hotkeys (M0, rebindable on keyboard and pad since M4): default F1 save state, F2/F3 previous/next slot, F4 load state, F11 fullscreen, F10 pause menu, Esc fixed (M3; Esc no longer quits: quit is in the menu behind a Yes / No question, or close the window / Alt+F4 with no prompt, SRAM flushed); slot + result shown in the window title and, for F1–F4, as an on-screen status ("Slot 3 · Saved", 5×7 pixel font on a dimmed box, bottom-left of the game viewport, 3 s) on the overlay layer (the core frame stays clean). Pad hotkeys fire on press edges (trigger crossings included) through one ordered event stream that reaches the pause menu first; hotkey-bound pad inputs never reach the core. Default pad hotkeys: right trigger fast-forward, left trigger rewind, the rest unbound.
- Fast-forward (held, `fast_forward_hold`): default `` ` `` key (physical key left of 1) or gamepad R2 past half travel (R2 is not an SNES button by default). While held the loop emulates uncapped with frame skip: for 50 ms per redraw the core runs with `GET_AUDIO_VIDEO_ENABLE` = 0 (snes9x skips PPU/APU rendering; frames it still sends are discarded), then one rendered frame is presented (~20 fps preview, few vsync waits); audio goes to a null sink, the title shows "Fast-forward"; on release the wall-clock pacer is reset (no catch-up), the drained audio queue is cleared and re-prefilled to target, and DRC resumes. The same fast-forward branch bypasses the §3.1 frame-due/DRC path.
- Turbo A (`turbo_a_toggle`, press; default keyboard P, pad unbound): toggles auto-fire of RetroPad A on port 0 (`input_mapping::TurboButton`, OR-ed into the snapshot at poll): 3 frames pressed / 3 released (`TURBO_HALF_PERIOD_FRAMES`). The phase counts real-timeline frames only (bumped in `RealTimeline::after_real_frame` and per fast-forward frame), so run-ahead's ahead frames all see the next real frame's phase (exact for run-ahead 1; at 2 the predicted picture may lag one frame at a phase edge, rolled back anyway); restarts with a press when switched on; off at session start, not saved. OSD "Turbo A: On" / "Off".
- Rewind (held, M3, `rewind_hold`; only with `rewind.enabled`): default Backspace or gamepad L2 past half travel. Every 2nd real-timeline frame (`REWIND_GRANULARITY`) the state (`RUNAHEAD_SAME_INSTANCE` context, reusing run-ahead's serialize of the same frame) goes into `RewindRing`: newest raw state + LZ4 blocks of `newer XOR older`, oldest evicted over `rewind.buffer_mb` (default 64 MiB ≈ 650 s on the test ROM). While held, run-ahead and fast-forward are bypassed; per capture due at core fps the ring steps back, the state is restored and one frame runs with video only; OSD "Rewinding" / "Rewind: start of buffer". Release resets pacing like fast-forward. Off for cores with `INCOMPLETE` quirk (OSD on first press) or after a serialize failure; a slot load clears the history.
- Save-state thumbnails (M3): after `slotN.state` + `slotN.json` are written, the clean core frame (no OSD, no shader) is PNG-encoded at native size on a `state-thumbnails` worker thread and written atomically as `slotN.png`; joined on exit. `--load-state N` restores slot N before the first frame (windowed and headless); hidden `--headless-save-slot N` saves after the last headless frame (tests).
- Resume point (M4): reserved pseudo-slot `auto` (`ipc_protocol::StateSlotId::Auto`, files `auto.state/json/png`, never touched by F1–F4 / slot cycling / the slot grid). `EmulationApp::finish()` saves it on every clean exit before the final SRAM flush when `resume.auto_save` is on, the window opened, the loop ended without error and ≥ 600 real-timeline frames ran (`auto_save_on_exit` policy; failure only logged, exit code unchanged; ~6–12 ms debug build). Sidecars (every slot) add optional `sram_crc32` (live SRAM = bytes flushed next) and `rom_patch_crc32` (CRC of the soft-patch chain found — files concatenated in order, so one patch = its file CRC — applied or not). `--load-state auto` / `--headless-save-slot auto`; a failed resume boots fresh with OSD "Could not resume" (success: "Resumed").
- Keyboard events come from winit (window focus); gamepads from SDL (`SDL_WINDOWS_ENABLE_MESSAGELOOP=0`; the video subsystem starts only for an OpenGL core, §3.3, and its hidden window's messages are pumped by winit). Both merge into one snapshot per frame (buttons + 4 axes per port).

### 3.6 Config layers (M3, `crates/config-layers`)
- Files under `<data>/config/`: `global.toml`, `system/<system>.toml`, `core/<core id>.toml`, `game/<system>/<rom key> [<ID>].toml` (same name as the save folder, §5.1). Precedence: CLI flags > game > core > system > global > built-in defaults. Merge = `toml::Table` deep-merge, then `#[serde(default)]` structs; an invalid value is dropped (warning naming the layer) and that key falls back; an unreadable / non-TOML layer is skipped.
- Menu option keys (`MENU_OPTION_KEYS`): `video.shader`, `video.scaling` (`aspect`/`integer`/`stretch`), `latency.run_ahead_frames` (0–2), `rewind.enabled`, `rewind.buffer_mb` (16–512). Defaults: shader `none`, `aspect`, run-ahead 0, rewind off, 64 MiB. The pause menu's "Save for this game / as default" and "Reset game settings" act on these keys only.
- Resume keys (M4): `resume.auto_save` (bool, default `true`, read by the runner), `resume.mode` (`ask` / `always` / `never`, default `ask`, read by the launcher).
- Binding sections (M4), all two-level so the flat dotted-key model is unchanged: `[input_keyboard]` / `[input_gamepad]` (actions `b y select start up down left right a x l r l2 r2` + 8 stick actions, M10), `[hotkeys_keyboard]` / `[hotkeys_gamepad]` (the 9 hotkeys). Value = array of 0–2 input names; `[]` = explicitly unbound, absent = inherit; arrays replace on merge, so a game override of one action leaves the others inherited. Built-in defaults (`defaults_table()`) are merged as the lowest layer and never written to `global.toml`; "Reset to defaults" deletes keys. `resolve_config_with_warnings` / `resolve_layers_with_warnings` also return conflict warnings; `set_layer_values` writes several keys in one validated, atomic write. Modules: `input_binding_names`, `input_binding_schema`, `input_conflicts`, `known_key_checks`, `resume_config_schema`.
- System id = the game's registry system (runner `--system`, launcher `games.platform_id`). Core id = `core_layer_id()` (lower-cased `library_name` on the runner, the registry `CoreSpec.id` of the game's system on the launcher: `snes9x`, `mesen`, `mgba`; a launcher/runner parity test per core keeps them equal). A core that plays several systems shares one `core/<id>.toml`; per-system settings go in `system/<id>.toml`. The game layer path has one owner, `ipc_protocol::game_config_path_for_game`.
- Writes (`set_layer_value`): atomic, unknown keys kept, empty tables pruned, comments **not** preserved (plain `toml`). The runner reads the layers only at start; the pause menu writes the game / global layer, the launcher writes global (Settings) and game (Detail → Settings). Concurrent writes: last write wins.
- Achievements (M15, `achievements_config.rs`): `[achievements]` `enabled` (true), `hardcore` (false), `sound` (true), `leaderboard_popups` (true), `patched_rom_fallback` (true, follow-up §3.7). **Global layer only**: `resolve_layers_with_warnings` strips the table from every system / core / game layer (warning); the launcher refuses a game-scope write and hides the keys from the game view. `hardcore` is honoured only while `achievements::hardcore_ui_enabled()` (§3.7).
- `nostyra-runner --print-config` prints the resolved JSON incl. binding sections and `resume` (needs `--core` + `--rom`); a launcher test asserts its own resolution (gameplay view ∪ input view) equals it, with a game override and a hand-made hotkey/game conflict.

### 3.7 RetroAchievements (M15, `crates/achievements` + runner `achievement_session/`, `achievement_overlay/`)
- **Library**: rcheevos v12.5.0 vendored unmodified, built with `cc` (defines `RC_CLIENT_SUPPORTS_HASH`, `RC_HASH_NO_ENCRYPTED`; MSVC `/wd4244 /wd4245`); hand-written `extern "C"` declarations, every `#[repr(C)]` mirror checked against C `sizeof` / `offsetof` by a probe library (`ffi_layout_tests`). The crate root is `#![deny(unsafe_code)]` with `#[allow(unsafe_code)] mod ffi` only; trampolines run under `catch_unwind`.
- **One thread rule**: `AchievementClient` (`!Send`) owns the `rc_client_t` on the session thread; every rc_client call, callback, event, memory read and log happens there. HTTP runs on one FIFO worker thread (`request_http_worker.rs`): `server_call` registers the C callback in a session-thread registry `RequestId → PendingCallback` and sends only `Send` data `(id, url, body)`; the worker returns `(id, status, body)`; results are drained before each `do_frame` / `idle`; late / unknown ids are dropped. Transport failure → status −2 with a short body (`offline` / `timeout`, shown as the error text). Badge / icon GETs are low-priority background jobs (run only while the request queue is empty), never through `server_call`. Tests use `RequestDispatch::Inline` (fake answers inside `server_call`, so a set is loaded before frame 1's `do_frame`).
- **Session start** (`achievement_session_start.rs`): `achievements.enabled` → credentials (`NOSTYRA_RA_USER` / `NOSTYRA_RA_TOKEN` read and **removed** from the process env at the top of `main`, else `<data>/achievements/credentials.json`) → `hash_request(spec, path, patched)` (no `ra_console_id` → no session; PS1 `.pbp` → "not available for this format" popup, session inert) → hardcore = `config.hardcore ∧ hardcore_ui_enabled()` (else `rc_client_set_hardcore_enabled(0)`) → user agent `Nostyra/<ver> (<OS> <maj>.<min>) <core stem>/<core ver> rcheevos/12.5` → token login → journal replay job on the worker. Debug builds read `NOSTYRA_RA_FAKE_SERVER=<dir>` (canned `<kind>.json` / `<kind>.status`, `game-hash.txt` = the only known hash) for process tests.
- **Set load waits for memory**: rc_client validates every memref once when a set loads, so `begin_identify_and_load_game` starts on the first frame hook whose regions are `Ready`. `LazyMemoryRegions` (before every `do_frame`): `rc_libretro_memory_init` over the map snapshot, else `SYSTEM_RAM` / `SAVE_RAM` (ids 0 / 2 only; VIDEO_RAM never; GB / GBC / GBA never fall back); rebuilt when the map generation or the ptr / size of ids 0 / 2 changes (mGBA sends maps during frame 1; GPGX shrinks SAVE_RAM); 300 frames without regions → "Achievements unavailable with this core" + unload. Reads are raw (no byteswap). The info callback reaches the core through a scoped thread-local (`InfoScope`), never a stored pointer.
- **Hashing** (`content_hash.rs`, `ffi/hash_*_reader.rs`): `rc_client_set_hash_callbacks` installs a Rust file reader (std::fs, Unicode paths) and CD reader over `disc-image` (`disc_track_access.rs`: track list for cue / gdi / CHD CD+GD+DVD / CDI / CSO / ISO, rc special tracks → `TrackSelector`, 2048-byte user data at absolute LBA incl. the GD-ROM 45000 offset). Cartridges hash the patched bytes the core runs; discs by path; `.m3u` → first disc; PSP `.pbp` whole file. Disc swap hashes the new disc itself (`content_hash`) and calls `rc_client_begin_change_media` (the identify-and-change variant ignores custom readers).
- **Events** (`achievement_event_routing.rs`, order per pump: request observations → completions → events): unlock → overlay popup + chime + IPC `AchievementUnlocked` + snapshot rewrite; load OK → pending progress applied, snapshot, IPC `AchievementsLoaded`, placard; leaderboard / tracker / challenge / progress → overlay (leaderboards run in hardcore only in v12.5.0); token rejected (login or load) → popup "please log in again in Settings" + IPC `AchievementsLoginExpired` + session stopped; server error / disconnect / reconnect → popups (deduped, queue cap 32).
- **Offline journal** (`pending_unlock_journal.rs`, `<data>/achievements/pending/<unix ms>-<rand>.json`: kind, ids, score, hardcore, game hash, user, unix time): written when the award / leaderboard request is observed **sent**, removed when its answer is delivered or permanently refused (well-formed JSON error < 500; "already unlocked" = delivered); retryable answers (−2, 408, ≥ 500, non-JSON) keep it; a token rejection keeps it and stops the pass. Leftovers are rebuilt with the `rc_api` award / submit builders (+ `seconds_since_unlock`) and replayed at session start (worker) and launcher start (background thread; skipped offline, logged out or while a game runs); other users' entries are kept.
- **Offline sessions** (follow-up, `offline_cache_transport.rs`, `offline_response_cache.rs`, `offline_cache_file_names.rs`, `offline_cache_state.rs`; runner `offline_session_watch.rs`): `runner_backend(ua, data_root)` always wraps the real (or fake) transport in `OfflineCacheTransport`. A 200 answer with JSON `"Success": true` to `login2` / `achievementsets` / `startsession` is stored in `<data>/achievements/offline-cache/<user>/` (`login2.json`, `sets/<m hash>.json`, `sessions/<m hash>.json`; user = URL-decoded `u`, lower-cased, kept to `[a-z0-9._-]` else `~` + hex; hash 1–64 hex chars; `write_atomically`, `login2.json` owner-only on Unix; reads capped at `RESPONSE_LIMIT_BYTES`). Requests without `m=` (`g=` path, `gameid`) are never cached. When inner returns −2 or a gateway status (502/503/504/521–525) the cacheable kinds are answered `200` from the cache; **sticky offline**: after the first such answer to any kind, cacheable kinds skip inner for the rest of the transport's life, while award / submit / ping / downloads always reach inner (so rc_client's own retry delivers them when the network is back). A cached `login2` is served only when the request token equals the cached `Token`; a cached `startsession` gets this user's pending journal unlocks for the hash merged in (`When` = unlock time). Cache miss while offline → −2 with `OFFLINE_NO_CACHE_BODY` (`offline: play once online first`) → popup "RetroAchievements: offline: play once online first"; a load failing with `RC_LOGIN_REQUIRED` after a shown login failure gets no second popup (`SessionRouter.login_failed`). `OfflineCacheState { any_served, set_served }`: `any_served` on any cached answer → `drop_to_softcore("offline")` once; `set_served` only for a cached set with `GameId > 0` or a cached session → popup "RetroAchievements: offline — unlocks will sync when online" once per session (a cached login alone never claims syncing). Both flags are checked twice per pump (after completions are drained and after routing), so the load placard already shows softcore. **`record_unlock`**: a delivered unlock is appended `{ID, When}` to the cached session (`HardcoreUnlocks` if `h=1`, else `Unlocks`; no duplicates; no-op without a cached session): by the decorator on a `Success:true` award (in session and runner replay) and by `replay_pending_unlocks` on `Delivered` (launcher flush, which bypasses the decorator). Cleanup in `credentials_file.rs`: `delete_credentials` (Log out, login expired) removes the whole `offline-cache/`; `save_credentials` clears it unless the previous credentials file is readable and names the same user (case-insensitive). Limits: offline detection waits for the connect timeout (10 s), and a queued journal replay adds one more, so the set loads ~10–20 s after an offline start (earlier triggers missed); an offline disc swap keeps the previous set (`gameid` not cached, `ChangeMedia` failure shows nothing); a cached set is as fresh as the last online start; the token stays usable offline until a replay sees it rejected.
- **Patched-ROM fallback** (follow-up, `achievement_session/patched_rom_fallback.rs`): `PatchedRomFallback::arm` runs in `AchievementSession::start` right after `hash_request` and arms only when the request carries patched bytes (cartridge with a soft patch) and `[achievements] patched_rom_fallback` is on; the original file is hashed there once, before the first frame. The first `(LoadGame, RC_NO_GAME_LOADED)` with a different original hash → `retry_with_original_rom`: remembers the patched hash, `client.set_hardcore_enabled(false)` (+ "Hardcore disabled: patched ROM" when it was hardcore; `follow_client_hardcore` lifts the gates), `begin_load_game(original)`, no placard, pending progress kept. Success → popup "RetroAchievements: patched ROM not recognised — using the original ROM's achievements (softcore)" before the loaded placard; a second `RC_NO_GAME_LOADED` → unknown placard with the patched hash. A patched hash RA knows loads directly (no retry). Offline, the retry is served by the offline cache like any other hash. Discs never carry patched bytes.
- **Progress**: `rc_client_serialize_progress_sized` → hex in the slot / auto sidecar (`StateSidecar.achievements_progress`); `SaveStateSlots::load` returns it; absent → `deserialize(NULL)` (reset); a load while the set is still loading (or not yet requested) keeps it as pending progress applied on load completion (last wins). The rewind ring stores one raw progress blob per capture (follows pops, counted in `rewind.buffer_mb`).
- **Hardcore gate** (behind `achievements::HARDCORE_UI_ENABLED = false`; debug `NOSTYRA_RA_HARDCORE_UI=1`): shared `HardcoreGate` (`hardcore_gate.rs`) checked first in `SaveStateSlots::load` (F4, menu) and in startup / `--load-state` loads (no file read, no core call); rewind captures nothing and plays nothing ("Disabled in hardcore"); `open_menu` asks `rc_client_can_pause` ("Pause available in N s"); a fixed / `--core-option` value refused by `rc_libretro_is_setting_allowed` or the registry's `CoreSpec.hardcore_required_options` (melonDS DS must be DS mode: rcheevos keys its table on "melonDS") → softcore for the session + popup. rc_client disabling hardcore (unknown disc), an unhashable swapped disc or a stopped client lift the gate. Saving, auto-save, fast-forward and run-ahead stay allowed. Launcher: `hardcore_effective` (flag ∧ enabled ∧ hardcore ∧ logged in ∧ RA system) → resume point `available=false` + "Hardcore: resume disabled" hint, slots marked unloadable, `play_game` drops `--load-state`.
- **Overlay** (`achievement_overlay/`): FIFO popup queue, one visible (placard 5 s, mastery 6 s, others 4 s, 32 waiting), placard on game load (counts + mode, unknown game + hash, no memory), unlock / mastery / leaderboard / notice popups with badge, bottom-right trackers + challenge badges + progress (hides 2 s after the last update); pause-menu **Achievements** page from `rc_client_create_achievement_list(CORE, PROGRESS)` (locked badge variant while locked). Badge cache `<data>/cache/achievement-badges/<badge>.png` / `<badge>_lock.png` / `game_<icon>.png`, fetched once per session for window sessions only (headless never downloads), kept only if the PNG decodes, LRU 256 decoded images, grey placeholder meanwhile.
- **Shutdown** (`achievement_client_shutdown.rs`): route arrived results → loop {fail every pending request with −2 `shutdown` → `rc_client_unload_game`} until none is pending (an award callback after destroy is a use-after-free) → `rc_client_destroy` → join the worker ≤ 2 s, else detach (a blocked ureq call holds only `Send` data). `achievements` is the first `EmulationApp` field, so even an unwinding drop releases it before the core session. The journal is already on disk.

## 4. Launcher ↔ Runner IPC (M1)
Contract in `crates/ipc-protocol`; launcher side in `crates/runner-session` (Tauri-free).
- Launcher spawns `nostyra-runner --core --rom --system <id> --content-sha1 <sha1> --data-dir <data> --ipc` (explicit args, no shell,
  no console window on Windows). M1 is stdout-only: newline-delimited JSON, each event written as
  `"\n" + json + "\n"` because cores print unterminated text; the decoder starts at `{"type"`.
- Runner → launcher: `Started{core,rom}` (game loaded / window open), `Stopped{play_seconds}`
  (after final SRAM flush), `Error{message}` (before non-zero exit), `CoreInfo` (`--probe-core`).
  Launcher → runner commands (`Pause`, `SaveState{slot}`…) are typed but not wired (runner stdin is
  null). M3 passes everything as start arguments: `--load-state N` (Detail → Save states → Load) and
  `--shaders-dir <resource_dir>/shaders`. M4 adds `--load-state auto` (resume point).
- Resume (M4, launcher `commands/resume_point.rs`, `resume_commands.rs`): `get_resume_info(gameId)` reads
  `auto.json` → `{ mode, available, createdAt, thumbnailPath, warnings }`; warnings `sram-newer`
  (`save.srm` CRC ≠ sidecar `sram_crc32`; a missing `save.srm` gives no warning), `patch-changed`
  (CRC of the patch chain next to the ROM ≠ `rom_patch_crc32`), `core-version-changed` (skipped when either
  version is unknown), `state-incompatible` (M14: `StateLoadRule::for_game` — registry `strict_state_version` + installed core version, or the sidecar `content_tag` ≠ the game's — through the shared `ipc_protocol::state_incompatibility`; `ResumeInfo.incompatibleReason` "made by another Dolphin version" / "made from another disc or revision"; the dialog hides Continue; strict cores never report `core-version-changed`; `SaveStateSlot.incompatibleReason` disables Load on that slot card). Every Play entry point goes through one path (`usePlayGame`): `never` or no
  resume point → plain launch; `always` without warnings → `--load-state auto`; otherwise the Resume
  dialog (Continue / Start fresh; Start fresh never deletes the point). A state from another core falls
  back to the runner's own check ("Could not resume", fresh boot). `list_save_states` returns the
  resume point first (`slot: "auto"`); `delete_save_state` accepts it.
- Controls (M4, `commands/input_catalog_commands.rs`, `input_binding_assign.rs`, `input_config_view.rs`):
  `get_input_catalog` (names, labels, reserved inputs, defaults from `config-layers`; no hand-kept TS
  list), `get_gameplay_config` returns bindings in a separate `input` view (so Detail "Reset all" never
  touches controls), `assign_binding` (steal, one layer write), `reset_controls`.
- Gamepad capture (M4, `gamepad_capture_thread.rs` / `_state.rs` / `_filter.rs`,
  `commands/gamepad_capture_commands.rs`): `capture_gamepad_input` (UI waits 5 s, capped at 10 s) runs on
  a dedicated thread that creates SDL (gamepad subsystem only, background joystick events allowed) for
  one capture and drops it after; one capture at a time, refused while a game runs, and Play cancels and
  waits for a running capture before spawning the runner (no SDL/HIDAPI sharing with the runner).
  Held-at-start inputs are ignored until released, triggers count at ≥ 50 %, Guide is ignored; result
  `{ input, labelStyle }` (Sony labels when the last captured pad is a PlayStation type). Keyboard
  capture runs in the page (`KeyboardEvent.code` → winit name); WebView2 swallows F10 / Alt / Win, so
  those keys are bindable only by editing `global.toml`.
- RetroAchievements (M15, `commands/achievements_commands.rs`, `achievements_game_view.rs`, `achievements_mode.rs`, `achievements_journal_flush.rs`): `ra_login` (password wrapped at once, one `rc_api` login request, stores `{user, token}` only), `ra_logout`, `ra_account`, `achievements_capabilities` (`{hardcoreUi}`), `game_achievements(gameId)` (`unsupported` / `loggedOut` / `notLoaded` / `ready` from the runner's snapshot `<data>/achievements/games/<sha1>.json`; unofficial achievements hidden). `play_game` passes `NOSTYRA_RA_USER` / `NOSTYRA_RA_TOKEN` as **child env** (never argv; `RunnerLaunch` `Debug` redacts the token) only when logged in and `achievements.enabled`. Runner events `AchievementsLoaded{ra_game_id, unlocked, total, hardcore}`, `AchievementUnlocked{id, hardcore}`, `AchievementsLoginExpired` → `SessionUpdate::Achievements` → UI event `achievements-changed {gameId}` (tab refetch) or credentials deleted + `ra-logged-out`. The badge cache folder is added to the asset scope at startup; data-folder moves include `achievements/` and `cache/`.
- Logs and bug reports (v0.1.0): both apps install their subscriber through `ipc_protocol::log_files::init_logging` (stderr + size-capped file in `<data>/logs/`, local timestamps, `NOSTYRA_LOG` > `RUST_LOG` > `info`; the runner skips the file for `--probe-core` / `--print-config`). `commands/log_commands.rs`: `open_logs_folder`, `create_bug_report` (`bug_report_zip`: 2 newest launcher + 5 newest runner logs scrubbed by `log_secret_scrub` + `system-info.txt` from `bug_report_system_info`; zip revealed with `tauri-plugin-opener`). UI: Settings → About → Logs row and a **Create bug report** action on game error toasts.
- Play time: on exit of a started session the runner writes `<data>/sessions/pending/<ms>-<pid>.json`;
  the launcher imports it (by `rom_path`, idempotent via `source_key`) after the session and at
  startup, so it survives the launcher closing mid-game.
- Exit codes 0/2/3/4/5/6 (6 = `EXIT_GRAPHICS`, `HostError::HwRenderUnavailable` → `SessionErrorKind::Graphics`, "This system needs OpenGL 3.3 graphics — update the graphics driver."); anything else, a Windows exception or a signal = `crashed`. stderr tail
  (20 lines) kept for error details; 15 s without `Started` = launch timeout (UI only).
- Core downloads are validated out of process with `nostyra-runner --probe-core <lib>`.
- Cores (M6): the launcher resolves the core of a game from its system (`system_catalog::core_for_system`);
  no core is required at first run. Play with the core missing fails with `{ kind: "core_missing", coreId }`
  and the UI offers the download (install prompt), as does Settings → Emulator cores. `get_core_status` /
  `download_core` take a `coreId`; download events carry it (one download at a time). The runner's size
  limits, saves folder and system config layer come from its `SystemSpec`; an unknown `--system` or an
  unclassifiable ROM exits 4 (ROM rejected).

## 5. Data Model (SQLite, M1+)

| Table | Key columns |
|-------|-------------|
| `platforms` | id (`snes`, `nes`…), name, extensions, dat_file, core_id — a mirror of the system registry (migration 003 adds `nes`, 004 `gb`/`gbc`/`gba` on `mgba`; a test checks every registry system has a row) |
| `cores` | id, platform_id, library_path, version, sha256, installed_at |
| `library_folders` | id, path, added_at (remove = games detached + hidden, history kept) |
| `games` | id, platform_id, folder_id, rom_path, file_size/mtime (rescan cache), crc32, sha1 (hashed bytes per the system's header rule: headerless for SNES, **whole file** for NES despite the "headerless" comment in `001_initial_schema.sql`), title, title_search, region, dat_name (NULL = needs review), missing |
| `game_metadata` | game_id, title_override(+_search), developer, publisher, genre, release_year, players, description, source_version, user_edited |
| `artwork` | (game_id, kind `cover`/`hero`), status `cached`/`not-found`/`failed`, local_path (relative to `<data>/artwork`), source, source_name, checked_at |
| `play_sessions` | game_id, started_at, duration_s, exit_code, source_key (idempotent import); view `game_stats` |
| `settings` | key, value |
| `collections`, `collection_games` | user collections (later; M2 favorites = `games.favorite`) |

Platform/core are data, not code → multi-system goal (G5). M1 implements `platforms`, `cores`,
`library_folders`, `games`, `play_sessions`, `settings` (`crates/storage-db`, migration 001); M2
adds `game_metadata`, `artwork`, `games.favorite` (migration 002); the rest arrive with M3.
M3 adds no table: save states live only on disk (`saves/<platform>/<rom> [<ID>]/slotN.*`; the runner writes
them without DB access, the launcher lists the folder, ≤ 10 slots, and deletes the three files,
refused while that game runs), and gameplay settings live in the config layer files (§3.6).
M4 adds no table: the resume point is the `auto.*` file trio in the same folder, and bindings live in the config layers.
Migrations are append-only. FK rules: removing a folder sets
`games.folder_id = NULL` + `missing = 1` (hidden, history kept); deleting a game cascades its
`play_sessions`, `game_metadata` and `artwork`. A moved/renamed ROM is relinked to its missing row by sha1.
Games are deleted only by "Remove from library" (Detail, or "Remove all N missing" on the Library
Missing filter), and only while `missing = 1` with a folder: the launcher first deletes the save folder
(SRAM, states, resume point) and game config layer — found by content id only (a legacy name-only entry
is never deleted), kept when another row has the same content id — and the cached art files, then the row; refused during a scan or while that game runs.

M2 metadata: scans write `game_metadata` from the vendored metadat (CRC32 join) for DAT-matched
games; rows the user edited (`user_edited = 1`) are frozen until "Reset to database values"
(delete + re-apply). A startup self-heal fills missing rows and, when the setting
`metadata_source_version` differs from the vendored commit, rewrites non-edited rows (rows written
from an incomplete index are stamped `<commit>+partial` and re-healed on the next full load).
Artwork rows: transient failures (offline, timeouts) write nothing, so the next job retries;
`not-found`/`failed` retry after 7 days. **`games.id` has no AUTOINCREMENT and artwork files are
named `<game_id>-cover.jpg` / `<game_id>-hero.png`: any future code that deletes games must delete
their artwork files first**, or a reused id would show another game's art.

### 5.1 Systems, classification and relink (M6, M7, M8, M9, M10, M11, M12, M13, M14)
- **Registry** (`crates/system-catalog`, std only): `SystemSpec` (id, labels, extensions, content probe,
  header rule, size limits, default core, DAT / metadat file, thumbnails repo/folder, pad actions, pad
  labels = the system's own button names per RetroPad action, e.g. Mega Drive `y` → "A") and
  `CoreSpec` (id, library stem, systems, licence, `fixed_options`, firmware, memory cards, `savestates`), plus
  `SystemSpec.analog_sticks` (M10: raw sticks; `AnalogSticks` enum since M13, §3.5) and
  `SystemSpec.touch_screen` (M13: pointer rule; M14 adds the aiming variant `WiiRemoteIr`, §3.5). `CoreSpec.savestates: SaveStateQuality` (the libretro
  `.info` field): `Deterministic`, or `Serialized` (serialize advances the emulation to a safe point, e.g.
  Mupen64Plus-Next) → run-ahead **and** rewind refused with the pause-menu reason "Core save states are not
  frame-exact" (same path as the `INCOMPLETE` quirk), the startup state load also retries core rejections
  (Mupen refuses `unserialize` before frame 1), and the host checks the exact state size before
  `retro_unserialize` (`CoreSession::set_exact_state_size`: Mupen never returns on a truncated state; a
  same-size corrupt state still hangs it). Slots, resume and thumbnails are unchanged. Every other crate reads it; no system or core literal elsewhere
  (`scripts/check-system-literals.*`, run in CI).
- **Classification**: one walk per library folder; per file extension → candidate systems → size limits →
  content probe on the first `PROBE_BYTES` = 0x200 bytes (0x110 before M13) (NES: `NES\x1A` at 0; GB/GBC: logo bytes
  `CE ED 66 66` at 0x104, the window's reason; GBA: ARM branch `0xEA` at byte 3) → first passing
  candidate. Folder names are never read; a file failing its probe is a scan failure ("missing iNES
  header", "missing Game Boy header", "missing GBA header"). `.gb` vs `.gbc` is decided by the
  extension (No-Intro naming), not the CGB flag. mGBA also wants `0xB2 = 0x96` in a GBA header; that
  second check is not probed (one magic per probe), so such a rare file is listed and fails at Play.
  `ContentProbe::MagicForExtensions` (M9) checks a magic only for some extensions of a system, at any of
  several offsets: Mega Drive `SEGA` at 0x100 / 0x101 only for `.bin` (shared with disc images) and `.md`
  (shared with Markdown); `.gen` / `.smd` / `.sms` / `.gg` / `.sg` go by extension. A miss on a "quiet"
  extension (`.md`) is `ClassifyError::NotRom` — the scanner ignores it silently like an unknown
  extension (a `README.md` is no game and no scan problem); a `.bin` miss falls through to the disc probe.
  `ContentProbe::MagicAny` (M10) accepts any of several magics at one offset: N64 `80 37 12 40` (z64),
  `37 80 40 12` (v64), `40 12 37 80` (n64) at 0 for `.z64` / `.v64` / `.n64` (the magic, not the extension,
  decides the byte order); a miss = scan failure "missing N64 header".
  Registry order is classification order: cartridges before discs (`ALL_SYSTEMS = [SNES, NES, GB, GBC,
  GBA, N64, NDS, GENESIS, SMS, GG, SG1000, PSX, DREAMCAST, PSP]`; discs shared by extension are probed PS1 → DC → PSP), so a `.bin` reads the cheap magic before the disc probe.
- **Hashing / identification**: SNES skips a copier header; Mega Drive SMD files (size `n × 16 KiB + 512`,
  no `SEGA` at 0x100; `HeaderRule::MegaDriveSmd`) hash as their de-interleaved raw image
  (`system_catalog::rom_image_for_hash`, the same rule Genesis Plus GX applies on load), so an `.smd`
  matches No-Intro and shares the `.md` dump's content id and save folder; N64 (`HeaderRule::N64ByteOrder`,
  `n64_byte_order.rs`) swaps `.v64` (16-bit) and `.n64` (32-bit) images to big-endian in place before hashing
  (`hash_rom_file` owns the buffer, no 64 MiB copy), so every order matches the DAT's `.z64` row and shares one
  content id; the core still gets the file's own bytes (it detects the order itself); GB/GBC/GBA/SMS/GG/SG-1000 hash
  the whole file; NES hashes the whole file and, on a DAT miss,
  the bytes after the header (+ trainer) against the DAT's `.unh` entries (re-headered dumps). Large
  cartridges (M13): with `HeaderRule::None` and a file over `STREAM_HASH_THRESHOLD` (32 MiB) `hash_rom_file`
  streams CRC32 + SHA1 (never the whole file in RAM: 4 hash threads × 512 MiB DS games) and returns an empty
  byte buffer (callers that need bytes use header rules, which never stream); the threshold is injectable
  (`hash_rom_file_with`) for tests. Cartridge serial (M13, DS only, keyed on `ContentProbe::NdsHeader`, no
  system literal): `nds_game_code` reads the 4-character game code at 0x0C (own 16-byte read, independent of
  hashing); it is stored in `games.serial`, and on a DAT hash miss `DatIndex::lookup_serial(code, size)` gives
  the DAT name / region / metadata (title stays the file stem, as for disc serial matches); rematch uses the
  stored serial. Metadat is
  joined via `DatEntry.metadata_crc32` (the headered `.nes` CRC of the game name). DAT + metadat load
  lazily per system (`game_library::SystemCatalogs`); startup warms the systems already in the library.
- **Two-pass scan** (`scan_library`): pass A walks every folder and flags vanished files missing, pass B
  hashes/upserts per folder — a file moved between folders relinks whatever the folder order. A missing
  folder root is reported and its rows are left untouched.
- **Per-game file names** (`ipc_protocol::game_folder_key`): save folder `saves/<system>/<rom key> [<ID>]/`
  and game config `config/game/<system>/<rom key> [<ID>].toml`, `<ID>` = first 8 hex digits (upper case)
  of the game's content sha1 (`games.sha1`: cartridge per header rule, disc = first data track of the first
  disc, so all discs of an `.m3u` game share one folder; soft patches never change it, so a patched game
  shares the original's saves). Lookup matches the ` [<ID>]` suffix only (case-insensitive, first by name
  if several), so renaming/moving the ROM or renaming the name part keeps the files; the `<rom key>` part is
  only for people. The launcher passes `--content-sha1 <games.sha1>`; the standalone runner hashes the ROM
  itself (`game_library::game_content_sha1`, same bytes as the scan). Resolving never creates an entry.
- **Legacy names** (`<rom key>` without id, before 2026-10-25): resolving renames one to `<rom key> [<ID>]`
  on first use (a failed rename keeps using the legacy path). At every launcher start, before any Play,
  `legacy_game_files_migration` matches legacy entries to games by `rom_key(rom_path)`: one content id →
  rename; several (different ROMs with the same file name shared the old folder) → copied to each, legacy
  kept and logged. Never deletes or overwrites.
- **Rename**: when a relink changes the ROM key (file stem), the launcher renames `<old key> [<ID>]` to
  `<new key> [<ID>]` (save folder and game config) after the scan (session lock held), unless the game is
  running, another row has the same content id, the target exists, or the name is not the app's
  (`<old key> [<ID>]` — a name the user chose is kept); a legacy `<old key>` entry gets its id on the way
  unless another game with that file name may share it. Files are never deleted or overwritten.
- **Adding a system**: (1) a `SystemSpec` in `system_specs.rs` + `ALL_SYSTEMS`; (2) its core in
  `core_specs.rs` (or the system id in an existing core's `systems`) + `scripts/fetch-<core>-core.*`;
  (3) its DAT name in `scripts/update-libretro-dat.*`, run it, commit the files; (4) a storage-db
  migration inserting the `platforms` row; (5) a self-written test ROM if the runner suites should cover
  it. Scanner, launcher, runner and UI need no change (UI labels and pad buttons come from `get_systems`).
  M7 example: `gb`, `gbc`, `gba` → one core `mgba` (`CoreSpec.systems` lists all three, one shared
  `core/mgba.toml`), migration `004_handheld_platforms.sql`, test ROMs `nostyra-test.{gb,gbc,gba}`.
  M8 example (first disc system): `psx` in `disc_system_specs.rs` (`Media::Disc`, disc probe), core
  `pcsx-rearmed` with `fixed_options`, `firmware`, `memory_cards`, Redump DAT source folder in the DAT
  script, migration `005_playstation_platform.sql` (+ `games.serial`), test discs `nostyra-test-psx*`.
  M9 example: `genesis`, `sms`, `gg`, `sg1000` in `sega_system_specs.rs` (GB/GBC/GBA moved to
  `handheld_system_specs.rs`) → one core `genesis_plus_gx` (`core/genesis_plus_gx.toml`), extension-scoped
  probe + SMD header rule, `pad_labels`, migration `006_sega_platforms.sql`, test ROMs
  `nostyra-test.{gen,sms,gg,sg}`.
  M10 example (first GPU core): `n64` in `nintendo_64_system_spec.rs` (`MagicAny` probe, `N64ByteOrder`
  hash rule, `analog_sticks`, N64 pad labels incl. C-buttons) → core `mupen64plus-next` with a mandatory
  `fixed_options` list (`core_fixed_options.rs`: unanswered keys fall back to 0 in the core = interpreter, dead
  stick, no pak) and `savestates: Serialized`, migration `007_nintendo_64_platform.sql`, test ROM
  `nostyra-test.z64` (Python-built raw MIPS ROM; libdragon IPL3 r5+ hangs Mupen's dynarec). The hw-render
  path (§3.3) needed no registry field: the core asks for its context at load.
  M11 example (second disc and GPU system): `dreamcast` in `dreamcast_system_spec.rs` (`DiscSignature::Dreamcast`,
  `analog_sticks` + `analog_triggers`, DC pad labels) → core `flycast` in `dreamcast_core_spec.rs`
  (`FLYCAST_OPTIONS`: every `reicast_*` key whose C fallback differs from its declared default, 1280×960,
  threaded rendering off, `All VMUs`), new `CoreSpec` fields `port_devices`, `exact_state_size` (Mupen only;
  replaces "Serialized ⇒ exact"), `bios_switch` (`BiosOptionSwitch`) and `FirmwareFile.{size, subdir}`
  (value types in `core_spec_types.rs`), Redump DAT + developer metadat, migration
  `008_dreamcast_platform.sql`, test program + discs `nostyra-test-dc*` (Python SH-4, ELF + GDI).
  M12 example (third disc and GPU system): `psp` in `psp_system_spec.rs` (`DiscSignature::Psp`, `iso cso chd
  pbp`, 8 GiB, `analog_sticks`, PSP pad labels) → core `ppsspp` in `psp_core_spec.rs` (all 70 `ppsspp_*`
  options: JIT, 960×544, frame duplication, frameskip off, `Cross` confirm, 0.15 deadzone, no texture
  replacement / cheats / network), `CoreSpec` moved to `core_spec_types.rs` with a `CORE_DEFAULTS` const
  (`..CORE_DEFAULTS` in every spec) and new fields `system_assets: Option<CoreAssets { dir, markers,
  archive_path }>`, `process_env`, `requery_state_size`, `max_state_bytes` (default 64 MiB, PPSSPP 128 MiB);
  Redump DAT + 5 metadat (maxusers placeholder), migration `009_psp_platform.sql`, test program + containers
  `nostyra-test-psp*` (Python, no PSPSDK).
  M13 example (first touch-screen system): `nds` in `nds_system_spec.rs` (`nds dsi ids`,
  `ContentProbe::NdsHeader`, 4 KiB .. 512 MiB, `touch_screen: DsBottomScreen`, `AnalogSticks::RightOnly`, NDS pad
  labels) → core `melonds_ds` (id = `core_layer_id("melonDS DS")`) in `nds_core_spec.rs` (44 fixed
  `melonds_*` options: DS mode, direct boot, built-in system files, SD cards off, software renderer threaded,
  layouts top-bottom / left-right, `show_cursor=touching`, `mic_input=blow`, firmware name "melonDS DS",
  every OSD off), `Deterministic`, `exact_state_size`, `max_state_bytes` 128 MiB, `process_env`
  `MELONDSDS_SKIP_ERROR_SCREEN=1`; No-Intro DS (Decrypted) DAT + metadat, migration `010_nds_platform.sql`,
  test ROMs `nostyra-test.nds` / `nostyra-test-save.nds` (Python ARM, `arm_emitter.py` shared with the GBA
  builder, no logo bytes).
  M14 example (fourth GPU core, first core shared by two disc systems): `gc` + `wii` in
  `gamecube_wii_system_specs.rs` (appended after PSP → `ALL_SYSTEMS` 16; `DiscSignature::GameCube` / `Wii`; GC
  `iso gcm ciso gcz rvz wia dol elf m3u` ≤ 2 GiB, Wii `iso ciso gcz rvz wia wbfs dol elf` ≤ 8 GiB; `Both` sticks, GC
  analog triggers, Wii `touch_screen: WiiRemoteIr`) → core `dolphin-emu` (`core_layer_id` keeps the dash) in
  `gamecube_wii_core_spec.rs` (30 of 112 `dolphin_*` options: single core, `Hardware`, EFB 1×, synchronous
  shaders, HLE IPL, no OSD / cheats / rumble / network, Remote + Nunchuk, `ir_mode=2`), `Serialized`,
  `requery_state_size`, 256 MiB cap, `system_assets` (`Dolphin.zip` → `<system>/dolphin-emu/Sys/`, markers
  `Sys/codehandler.bin`, `Sys/GC/font_western.bin`), new flags `hw_render_required`, `strict_state_version`,
  `keep_library_loaded` (§3.3); Redump GC + Wii DATs + metadat, migration `011_gamecube_wii_platforms.sql`, test
  programs `nostyra-test.dol` / `nostyra-test-wii.dol` (Python PowerPC, `ppc_emitter.py`; Wii = the same code +
  one `mtspr HID4` word).
- **Disc systems (M8, PlayStation on PCSX ReARMed)** — `SystemSpec.media = Media::Disc` switches every
  disc path:
  - *Probe*: `ContentProbe::Disc(DiscSignature)` is data in the std-only registry; the `DiscProbe` trait is
    implemented by `disc_image::DiscImageProbe` (scanner + runner; `NoDiscProbe` elsewhere). The classifier
    reads a file head only for a size-fitting magic candidate and hands disc candidates to the probe.
    PlayStation = ISO9660 system id `PLAYSTATION` **and** "Sony Computer" licence text in sector 4 (PS2 CDs
    have no licence text); `.cue` / `.m3u` first check that every referenced file exists (one readable
    reason, e.g. "track file 'X' not found — fix the FILE line or rename the file"), then probe the first
    data track / first disc; `.pbp` needs SFO `CATEGORY = ME` + `DISC_ID`; `.chd` via the `chd` crate.
  - *Grouping* (`game_library::disc_file_grouping`, once per library root after the walk): a `.cue` / `.m3u`
    is the game; every file a sheet references (tracks; a playlist's discs and their tracks, any folder) is
    hidden together with its walk failures (audio tracks fail the probe); a same-stem `.bin` beside a cue
    and, beside a broken cue, bins whose stem starts with the cue's or a missing `FILE`'s stem are hidden
    too — these stem rules only hide files classified as a disc system, never a cartridge (a Mega Drive
    `Sonic.bin` beside an unrelated `Sonic.cue` stays a game). Fingerprint = sum of sizes, newest mtime.
  - *Identity*: streaming CRC32 + SHA1 (1 MiB chunks) of the first data track file (what Redump hashes;
    `.chd` / `.pbp` hash the container, so they match by serial only) + boot serial from `SYSTEM.CNF`
    (`SLUS-01234`; PBP: SFO). DAT match: hash → DAT title; else serial (`DatIndex::lookup_serial`, size
    tie-break) → DAT name / region / art / metadata but the file's own title. PS1 metadat is one
    serial-keyed file (`MetadataIndex::lookup_serial`); `games.serial` (migration 005) feeds rematch and the
    metadata self-heal / reset.
  - *Runner*: no soft patching; `validate_rom` re-runs the disc probe (track deleted since the scan → exit
    4 with the same reason); fixed core options from `CoreSpec.fixed_options` (+ hidden `--core-option`)
    answered via `GET_VARIABLE`; disk control (EXT / legacy) → `CoreSession::discs()` and a pause-menu
    **Disc** item for multi-disc games (eject → index → close; states keep the index); core messages
    (`SET_MESSAGE(_EXT)`) → log + OSD (same text once per 5 s); one BIOS log line per start
    (`firmware_status`: region file → any valid BIOS → HLE).
  - *Memory cards*: `CoreSpec.memory_cards` — PCSX card 1 = `SAVE_RAM` (`save.srm`, 128 KiB), card 2 = the
    core's own `pcsx-card2.mcd` in the game's save folder; the launcher imports `.mcr` / `.mcd` / `.srm` /
    `.gme` / `.vgs` cards into a slot (backup `<file>.before-import-<date>`).
  - *Quirks*: PCSX ReARMed bumps a counter inside the state on every serialize (rewind checks tolerate
    `state_jitter_bytes`); the core sets up no memory card unless `memcard1/2` are answered (fixed options).
- **Dreamcast discs (M11, Flycast)** — the second disc system reuses every M8 path and adds:
  - *Formats* (`disc-image`): `.gdi` (`gdi_sheet.rs`, strict: count line + `n lba ctrl size "file" offset`,
    quoted names, 2048 / 2352 sectors), Redump `.cue` with `REM SINGLE-/HIGH-DENSITY AREA` (`TrackArea`), CHD
    with `CHGD` / `CHGT` GD-ROM track metadata (`chd_gd_tracks.rs`: tracks padded to 4 frames, any track
    readable), DiscJuggler `.cdi` v2 / v3 / v3.5 (`cdi_image.rs` + `cdi_track_table.rs`, Flycast's `cdipsr`
    layout; other versions "unsupported DiscJuggler version"). `.gdi` is a sheet (`is_sheet_ext`).
  - *IP.BIN* (`dreamcast_disc_id.rs`, `dreamcast_ip_track.rs`): sector 0 of the boot data track (GD-ROM: first
    data track at LBA ≥ 45000; cue: first high-density data track; CHD: GD track 3 / MIL-CD data session; CDI:
    last session's first data track): hardware id `SEGA SEGAKATANA`, product, version, disc, area, title.
    `DiscImageProbe` checks it per signature; a `.cue` / `.chd` / `.m3u` is tried as PlayStation first (fails
    fast on a GD-ROM), and neither system claims the other's discs.
  - *Identity* (`identity_source(path, signature)`, one rule for scan and runner content id): Dreamcast `.gdi` /
    `.cue` hash the **largest data track** (the one rom the Redump DAT lists), `.chd` / `.cdi` the whole file,
    `.m3u` its first disc; serial = IP.BIN product (display form `MK-51064`). Serial matching
    (`disc_serial_rule.rs` + `game_library::serial_match_keys`): exact text first (PS1 unchanged), then the
    signature's keys (DC: alphanumerics, leading `MK` dropped, one-digit disc suffix dropped, comma lists split)
    on both the DAT / metadat side and the disc side.
  - *Grouping*: a `.gdi` hides its track files; an `.m3u` may list `.cue` / `.gdi` / `.chd` / `.cdi` discs.
  - *Runner*: port devices after load (§3.5), `exact_state_size: false` (Flycast's 37 748 736-byte states are
    constant but an upper bound), BIOS switch (`bios_option_switch.rs`: `<system>/dc/dc_boot.bin` with the
    registry's size + md5 → `reicast_hle_bios=disabled`, else `enabled` + log; Flycast boots any file with that
    name and dies on a bad one), a rejected slot / resume state re-applies a snapshot taken just before (Flycast
    stays stopped after a failed `retro_unserialize`), VMUs `<product>.A1.bin` in the game's save folder
    (`GET_SAVE_DIRECTORY`), flash `dc_nvmem.bin` in `<system>/dc/`. Flycast's `retro_reset` reloads the
    playlist's first disc. Settings → BIOS lists Flycast with one "All regions" row; a file counts only with a
    known md5 (same rule as the runner) and import stores it as `dc/dc_boot.bin`.
- **PSP UMDs and eboots (M12, PPSSPP)** — the third disc system reuses the M8 / M11 paths and adds:
  - *Formats* (`disc-image`): ISO9660 lookup by path through nested, multi-sector directories
    (`iso9660_root.rs`), CISO v1 `.cso` (`cso_image.rs`: deflate / plain blocks, index read per entry, flate2;
    ZSO / CSO v2 refused with a reason), DVD CHD from `chdman createdvd` (`chd_dvd_reader.rs`: 2048-byte units,
    no CD / GD track metadata), `PARAM.SFO` (`param_sfo.rs`, shared with eboots) and PBP kind
    (`pbp_sfo.rs`: PS1 classic or PSP eboot).
  - *Disc id* (`psp_disc_id.rs`): ISO9660 system id `PSP GAME` or a `PSP_GAME` directory; serial from
    `UMD_DATA.BIN` or `PARAM.SFO` `DISC_ID`; title from `PARAM.SFO`; UMD Video / Audio refused with a reason.
    A `.iso` / `.chd` / `.pbp` is probed PS1 → DC → PSP; no system claims another's images. No `.m3u`.
  - *Identity*: content id = the whole file (`.iso` matches the Redump DAT by SHA1; `.cso` / `.chd` / eboots by
    serial). PSP serial keys on both sides: `ULUS10041` ↔ `ULUS-10041`, DAT `ULES-01513/E` found by
    `ULES-01513`. An eboot without a hash match is titled from `PARAM.SFO` `TITLE` (else its folder name). The
    clrmamepro parser also reads rom-level `serial` (PSP releaseyear metadat).
  - *System files*: `CoreSpec.system_assets` — core-manager downloads the core and `assets/system/PPSSPP.zip`
    as one install (one progress bar, all or nothing, zip-slip safe, 64 MiB cap) into `<cores>/PPSSPP/`
    (`core_assets_installer.rs`; `scripts/fetch-ppsspp-core.*` does the same for the repo's `cores/`). Before
    load the runner (`core_system_assets.rs`) installs them into `<system>/PPSSPP/` when a marker is missing or
    the first marker (`compat.ini`) differs by content from the core's copy (staged replace,
    `config_layers::staged_dir_replace`); none anywhere → exit 3 "PPSSPP system files are missing (<path>).
    Re-download the core in Settings → Cores.".
  - *Runner*: `CoreSpec.process_env` set before the core loads (`CI=1`: PPSSPP release asserts would open a
    modal dialog; `.cargo/config.toml` sets it for tests); `validate_rom` → exit 4 for a non-PSP image forced
    with `--system psp`. Memory stick = the game's save folder (`GET_SAVE_DIRECTORY`): `PSP/SAVEDATA/…`,
    `PSP/GAME/<DISC_ID>/` (updates / DLC), `PSP/NAND/flash0/font/`; no `save.srm`; every eboot's folder is
    `EBOOT [<id>]`.
  - *States*: `Serialized` (run-ahead / rewind refused). `requery_state_size`: the host asks
    `retro_serialize_size` on every save (128 MiB before the asynchronous boot, 40–72 MiB after), capped by
    `max_state_bytes` (128 MiB). Startup resume waits for the boot (`startup_boot_gate.rs`: state size change,
    at most 10 s) and makes one attempt (a bad state = "Could not resume", never "Resumed"); a slot load before
    the boot shows "Core not ready" (`pre_boot_load_guard.rs`).
- **Nintendo DS cartridges (M13, melonDS DS)**:
  - *Probe* (`nds_header_probe.rs`): header CRC-16/MODBUS over 0x000–0x15D == u16 @0x15E, ARM9 entry in
    0x02000000–0x023BFE00, ARM9 / ARM7 ROM offsets ≥ 0x200, unit code @0x12 ∈ {0, 2}; unit 3 → scan problem
    "DSi-only game — not supported" (`NDS_DSI_ONLY`); other misses "missing DS header", "DS header checksum
    mismatch", "invalid DS header". No logo bytes are compared.
  - *Encrypted pre-check* (runner `runner_exit_codes/nds_rom_check.rs`, from `validate_rom` for `NdsHeader`
    systems): mirrors melonDS DS `console.cpp` + melonDS `IsHomebrew` — not homebrew (ARM9 offset ≥ 0x4000, no
    upper bound, game code ≠ `####`) and the secure area not decrypted (first ARM9 word ≠ `0xE7FFDEFF`, or the
    word at +0x10 equal to it) → `RomValidationError::Unplayable` → exit 4 "This DS dump
    is encrypted; Nostyra plays decrypted dumps (No-Intro Decrypted)"; unit code 3 → the DSi-only reason. The
    scan still lists encrypted dumps (they are DS games).
  - *Process env*: `CoreSpec.process_env` goes through `libretro_host::set_process_env`
    (`process_environment.rs`): `std::env::set_var` plus, on Windows, msvcrt's `_putenv_s` via
    `Library::open_already_loaded("msvcrt.dll")` — the MinGW-built core reads msvcrt's own environment copy,
    which `set_var` (UCRT / Win32) does not update. `MELONDSDS_SKIP_ERROR_SCREEN=1` makes a core config error
    fail `retro_load_game` instead of showing the core's error screen.
  - *Load error text (all systems)*: `HostError::LoadGameRejected(Option<String>)` carries the core's reason —
    its last on-screen message of the load (`SET_MESSAGE` / `_EXT`), else the last error-level log line
    (`environment_core_messages.rs`; melonDS DS's "Skipping error screen…" notice is never kept as a reason).
    Display appends `: <reason>`; the exit code stays 4. The launcher's exit-4 headline is system-neutral
    ("This game file could not be loaded…"; `runner_exit_classification.rs`).
  - *Saves / states*: `SAVE_RAM` → `save.srm` (8 KiB EEPROM … NAND; 0 for homebrew → no file), pointer + size
    re-queried at every flush (the core may re-allocate after a state load); loading a state also rewrites the
    in-game save (core design). States ~18–20 MiB, exact size, stable across reset; size 0 (DSi / error) →
    `SerializeUnsupported` before calling the core and the status "Save states are not available for this
    game" (`state_failure_status.rs`, also for `--load-state N`). Run-ahead / rewind allowed (picture-exact with
    the threaded renderer), but one run-ahead tick costs ~18 ms (unserialize ~11 ms). The core writes
    `<system>/melonDS DS/wfcsettings.bin` (every unload / reset) and an empty `melonDS DS/` in the save folder.
- **GameCube / Wii discs (M14, Dolphin)**:
  - *Header readers* (`disc-image` `nintendo_disc_containers.rs`, `nintendo_disc_header.rs`; generic over
    `Read + Seek`, ≤ 64 KiB read, proven with a counting wrapper): the first 0x80 header bytes of plain ISO / GCM
    (@0), CISO (`CISO` magic + `.ciso` ext, data @0x8000; PSP `.cso` keeps its reader by extension), WIA / RVZ
    (raw header copy @0x58), WBFS (@`1 << hd_sector_shift`), GCZ (block 0 inflated with `flate2`, or stored).
    The magic decides the system, never the extension: GC `C2339F3D` @0x1C, Wii `5D1C9EA3` @0x18. `.dol` /
    `.elf` (`dol_elf_executable.rs`): DOL section table / ELF32 big-endian PowerPC sanity, then a scan of the
    text sections (≤ 32 MiB) for `mtspr HID4` (Wii-only) or a section in MEM2 → Wii, else GC.
  - *Probe / classifier*: GC / Wii are the last disc candidates (PSX / PSP probes fail after one ISO9660 read).
    Reject texts depend on the extension (`.iso` "not a PlayStation, PSP, GameCube or Wii disc"; `.wbfs` "not a
    Wii disc"; other GC/Wii-only extensions "not a GameCube or Wii disc"; `.m3u` "not a PlayStation, Dreamcast or
    GameCube playlist"). `ClassifyError::outranks` (`classify_error.rs`): a later generic "not a … disc /
    playlist" reject never replaces an earlier specific reason (PSP "UMD Video", "not a valid GDI sheet" …). A
    valid ELF that is not ELF32 BE PowerPC is no GC/Wii candidate → quiet `NotRom` ("not a PowerPC executable",
    `foreign_executable_probe.rs`, head-byte pre-check); a malformed PowerPC ELF / DOL stays a problem row.
  - *Identity* (`game-library` `gamecube_wii_identity.rs`, first branch of `read_disc_identity`): **no
    whole-file hash**. Serial = 6-char game ID (`GALE01`); DAT lookup key = the 4-char code on both sides
    (`disc_serial_rule`: Redump `DL-DOL-GALE-USA` / `RVL-RJ2E-USA` → `GALE` / `RJ2E`); content id (`games.sha1`,
    save-folder key) = SHA1 of `"<sys>:<ID6>"` (no disc byte 0x06, no revision byte 0x07: discs, revisions,
    ISO ↔ RVZ and the `.m3u` share one save folder). Only the header + `fs::metadata` are read; the runner's
    `game_content_sha1` takes the same path. `.dol` / `.elf` = streaming file hash, no serial. Known limits:
    a standalone disc-2 / Rev 1 row shows the first DAT entry's name (same code, same full size); relink picks
    the lowest-id `missing` row with the same sha1, so moving several discs / revisions can swap rows.
  - *Saves*: no SAVE_RAM, no `memory_cards`; the core writes its own `User/` tree into the per-game save folder
    (`GET_SAVE_DIRECTORY`): `User/GC/<region>/Card A/` (GCI folder), `User/Wii/` (NAND), `User/Cache/Shaders/`.
    `dolphin_cheats_import=disabled` (else `<data>/cheats/` litter). Stale-staging cleanup never touches
    `User/**`.
  - *States + guard (all cores)*: `StateSidecar` gained optional `state_len`, `state_crc32` and `content_tag`
    (serde defaults; old sidecars load unchecked). Every save stamps them; every load runs
    `state_file_guard.rs`: core name → version (lenient: warn; `CoreSpec.strict_state_version`: refuse "made
    by another Dolphin version") → content tag (`game_library::state_content_tag`: GC/Wii disc `ID6:disc:rev`,
    `.m3u` `m3u:ID6`, else `None`; mismatch "made from another disc or revision", untagged sides pass) → cap
    (`max_state_bytes`) → length → read → CRC → core (Dolphin accepts or truncates any blob silently). Sidecar is
    written first: a crash between the two writes reads as "state file is damaged", never reaches the core.
    `SaveStateSlots::for_game` takes the strict flag from the registry and the tag from `RunnerDataPaths`. The
    launcher applies the same rule (`state-incompatible`, §4). States ~91–116 MiB (`requery_state_size`;
    size 0 before frame 1 → existing startup hold).
- **Battery memory** (runner `sram_persistence`): `SAVE_RAM` → `save.srm`, and `RETRO_MEMORY_RTC` →
  `rtc.bin` when the core keeps a cartridge clock apart from SRAM (mGBA's Game Boy MBC3 games: 48 bytes,
  restored before the first frame so the in-game clock advances by the time the game was closed; GBA
  cartridge clocks read the host clock, nothing stored). Both load right after `retro_load_game`, flush
  every 60 s if changed and on exit, at the core's size at that moment: mGBA reports a 128 KiB GBA save
  buffer until the game first touches its save chip, so a shorter file just fills its start; a longer one
  is kept as `<file>.bak` before the next save would trim it. Genesis Plus GX reports 64 KiB before the
  first frame, then only up to the last non-0xFF byte (2 bytes for the test ROMs; 0 = untouched → no file
  written), so `save.srm` is small and loads as a prefix. Mupen64Plus-Next exposes one 296 960-byte
  SAVE_RAM (EEPROM at offset 0, 4 controller paks, SRAM, Flash; RetroArch layout), formatted inside
  `retro_load_game` and loaded after it.
- **Staged content (M9)**: a core that loads content by path (`need_fullpath`, e.g. Genesis Plus GX)
  ignores preloaded bytes. The runner preloads a cartridge when it is soft-patched or its path has 256+
  UTF-8 bytes (the core truncates longer paths); libretro-host (`SessionConfig.staging_dir`) then writes
  the bytes to `<data>/temp/staged-content/<pid>/content.<ext>` (same extension: the core picks the
  system by it) and passes that path; the file is deleted after unload, and folders of crashed runners
  older than 1 h are removed at the next runner start. A staged path of 256+ bytes itself fails with
  `HostError::StageContent` (exit 4). Discs are never preloaded.

## 6. File Locations (per-user, via `directories` crate)
Root = `ipc_protocol::data_root()`: the folder chosen in Settings → Data folder, stored as one UTF-8 path in `ProjectDirs.config_dir()/data-location.txt` (`%APPDATA%\Nostyra\config\data-location.txt`, outside the root so it survives a move), while that folder exists; else the default: `<exe dir>/data` in a portable copy (`portable.txt` beside the exe, `ipc_protocol::portable_dir`; the location file then also lives beside the exe), otherwise `ProjectDirs("", "", "Nostyra").data_dir()` (`%APPDATA%\Nostyra\data`, `~/Library/Application Support/Nostyra`, `~/.local/share/nostyra`). Runner `--data-dir` overrides it (the launcher always passes it). Changing the folder (`data_folder_commands`) refuses while a game/scan/artwork fetch/core download runs, optionally moves the entries below (`data_folder_move::DATA_ENTRIES` only, so a shared folder keeps other files): DB connection closed, everything copied first (a failed copy reopens the old DB, nothing changes), location written, copied originals deleted; existing target files are overwritten only on request (else kept, their originals stay behind). The app then restarts. `cores.library_path` (absolute, unread) goes stale after a move. `<rom>` = ROM file stem, Unicode kept, Windows-forbidden chars → `_`, reserved device names suffixed; the per-game folder/file is `<rom> [<ID>]` where `<ID>` is the content id (see 5.1 "Per-game file names"), so same-named ROMs with different content do not share saves. The name is computed by `ipc_protocol::game_folder_key` and `rom_key` (shared by runner and launcher: `saves_dir_for_game`, `game_config_path_for_game`). M0 creates only `saves/` and `system/`.
```
<root>/
├── nostyra.db    # M1+
├── cores/        # M1+: downloaded/bundled cores
├── config/       # M3: global.toml, system/<id>.toml, core/<id>.toml, game/<platform>/<rom> [<ID>].toml
├── sessions/pending/ # M1: runner play-session records awaiting import by the launcher
├── temp/staged-content/<pid>/ # M9: content.<ext> staged for a path-loading core while a game runs
├── saves/        # <platform>/<rom> [<ID>]/ = one folder per game: save.srm, rtc.bin (cartridge clock, M7), slotN.state + slotN.json {core_name, core_version, created_at, sram_crc32?, rom_patch_crc32?, state_len?, state_crc32?, content_tag? (M14)} + slotN.png thumbnail (clean core frame); auto.* = resume point (M4); User/ = Dolphin's GCI memory card, Wii NAND and shader cache (gc / wii, M14)
├── achievements/ # M15: credentials.json {user, token} (plaintext, 0600 on Unix), pending/<ms>-<rand>.json offline journal, games/<sha1>.json runner snapshot for the launcher tab, offline-cache/<user>/ last server answers (login2.json incl. the token, sets/<hash>.json, sessions/<hash>.json; removed on logout / login expiry / account switch)
├── cache/achievement-badges/ # M15: <badge>.png, <badge>_lock.png, game_<icon>.png (shared by runner and launcher)
├── logs/         # v0.1.0: launcher-<YYYYMMDD-HHMMSS>.log per launcher start, runner-<stamp>-<system>.log per game session (10 MiB cap), nostyra-bug-report-<stamp>.zip; cleanup at every start keeps 10 launcher + 30 runner logs, drops logs > 14 days and zips > 7 days (`ipc_protocol::log_files`); not moved by a data-folder change
├── artwork/      # M2: <game_id>-cover.jpg (≤ 600 px, JPEG q85), <game_id>-hero.png (snap/title), index/<system>-tree.json(+.etag) per system; served via asset:// (scope = this folder + `saves/` for state thumbnails); clearable in Settings
└── system/       # core system dir (BIOS/firmware if a core needs it); dolphin-emu/Sys/ = Dolphin's system files, copied from <cores>/dolphin-emu/ (M14); melonDS DS/ = wfcsettings.bin written by melonDS DS (M13); PPSSPP/ = PPSSPP's system files, copied from <cores>/PPSSPP/ (M12); dc/ = dc_boot.bin + dc_nvmem.bin (Flycast, M11); Mupen64plus/ = Mupen64Plus-Next's mupen64plus.ini + GLideN64 shader cache (M10)
```

## 7. Platform Notes
- **Windows**: DX12/Vulkan via wgpu; MSI/NSIS via Tauri bundler.
- **OpenGL cores (M10)**: need an OpenGL 3.3 driver (the hidden GL context may sit on an iGPU of a hybrid laptop: CPU readback cost only). macOS has no 3.x compatibility profile, and Mupen asks for a compat context → GLideN64 likely fails there (open question: map to a 3.3 core forward-compatible context); SDL windows are main-thread only on macOS, so the in-binary GL suite skips there. Linux CI runs the GL tests on Mesa llvmpipe (`SDL_VIDEO_DRIVER=offscreen`, `NOSTYRA_REQUIRE_GL=1`).
- **Dolphin tests (M14)**: Null-renderer suites (`dolphin_renderer=Null`: boot, MEM1 field counter, pad words, audio, clean state refusal) run on every OS; the GL suites (pixels and every state / slot / guard / strict-version / content-tag / resume suite) are GL-gated, so they run on Linux CI (llvmpipe) and a dev GPU but **not on Windows CI** (no GL 3.3 there; accepted) and not on macOS (SDL main-thread limit). Suites stay in-process in the runner test binary (keep-loaded makes several Dolphin sessions per process safe); process tests that spawn a Dolphin runner use `support::output_within` (deadline, kill on timeout). macOS arm64 JIT needs the `allow-jit` entitlement for a hardened runner (unverified).
- **macOS**: Metal; universal or per-arch builds; cores must be signed/notarized.
- **Linux/Steam Deck**: Vulkan; Flatpak primary (deferred, Later); gamescope single-fullscreen-window behavior must be validated (launcher may hide while runner active).
