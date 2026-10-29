//! `--headless-frames N`: runs the core with no window, audio or input (CI has
//! no display/audio device). Loads/saves SRAM like a normal session so tests can
//! check persistence; honours `--load-state` (before frame 1, same retry rule as the
//! window) and the test-only `--headless-save-slot N|auto` (after the last frame,
//! through the same `GameSaveData` slot + thumbnail code as F1 and auto-save on exit;
//! headless never runs the exit auto-save itself). An achievement session gets the
//! same hooks as the window loop (frame, state progress, shutdown before the core
//! unloads); its status texts go to the log. Then prints a
//! machine-readable result line on stdout:
//! `frames=<n> checksum=<hex> nonblack=<bool> loaded=<bool>`.

use ipc_protocol::StateSlotId;
use libretro_host::{
    CoreSession, NullAudioSink, NullInputSource, NullVideoSink, VideoFrame, VideoSink,
};
use video_renderer::RgbaFrameBuffer;

use crate::achievement_session::{self, progress_of, AchievementSession};
use crate::game_save_data::GameSaveData;
use crate::ipc_event_writer::write_stdout;
use crate::process_hard_exit::teardown_watchdog::TeardownGuard;
use crate::rom_soft_patching::SoftPatchedRom;
use crate::runner_cli_args::RunnerCliArgs;
use crate::runner_core_options;
use crate::runner_data_paths::RunnerDataPaths;
use crate::startup_state_load::StartupStateLoad;

/// Feeds both the checksum sink (determinism) and an RGBA buffer (black check).
#[derive(Default)]
struct HeadlessVideo {
    checksum: NullVideoSink,
    rgba: RgbaFrameBuffer,
}

impl VideoSink for HeadlessVideo {
    fn frame(&mut self, frame: &VideoFrame<'_>) {
        self.checksum.frame(frame);
        self.rgba.frame(frame);
    }

    fn dupe(&mut self, width: u32, height: u32) {
        self.checksum.dupe(width, height);
    }
}

/// `guard` (declared after `session`: parameters drop in reverse order) arms the
/// teardown watchdog before the session unloads: with 0 after a clean run, with its
/// own code on an early `?` return.
pub fn run(
    mut session: CoreSession,
    guard: TeardownGuard,
    (frames, mut achievements): (u32, Option<AchievementSession>),
    args: &RunnerCliArgs,
    paths: &RunnerDataPaths,
    patched: &SoftPatchedRom,
) -> anyhow::Result<()> {
    let mut saves =
        GameSaveData::load(paths, &mut session, args.start_slot()).with_soft_patch(patched);
    let hardcore = achievements.as_ref().map(AchievementSession::hardcore_gate);
    saves.set_hardcore_gate(hardcore.unwrap_or_default()); // loads refused in hardcore
    let quality = runner_core_options::savestate_quality(paths.system);
    let mut pending_load = args
        .load_state
        .map(|slot| StartupStateLoad::new(slot, quality));
    let mut loaded = false;
    let mut video = HeadlessVideo::default();
    let (mut audio, mut input) = (NullAudioSink::default(), NullInputSource);
    for _ in 0..frames {
        if let Some(load) = pending_load.as_mut() {
            if let Some(outcome) = load.attempt(&saves, &mut session) {
                loaded = outcome.loaded;
                pending_load = None;
                if let Some(achievements) = achievements.as_mut().filter(|_| loaded) {
                    achievements.apply_loaded_progress(outcome.progress.as_deref());
                }
            }
        }
        session.run_frame(&mut video, &mut audio, &mut input)?;
        achievement_session::after_real_frame(&mut achievements, &session);
        for notice in achievements
            .as_mut()
            .map(|a| a.take_notices())
            .unwrap_or_default()
        {
            tracing::info!(%notice, "achievements");
        }
    }
    if let Some(load) = pending_load {
        tracing::error!(
            slot = %load.slot(),
            "--load-state: core not ready before the last frame"
        );
    }
    if let Some(id) = args.headless_save_slot {
        if let StateSlotId::Numbered(slot) = id {
            saves.select_slot(slot); // as F1 on that slot
        }
        let progress = progress_of(&achievements);
        if let Err(err) = saves.save_slot(&mut session, id, &video.rgba, progress) {
            tracing::warn!("save state failed: {err:#}");
        }
    }
    saves.flush(&mut session);
    if let Some(achievements) = achievements.take() {
        achievements.shutdown(); // before the core unloads (D4)
    }
    drop(saves); // joins the thumbnail worker: the PNG is on disk before the result line
    let nonblack = video
        .rgba
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .any(|px| px[..3] != [0, 0, 0]);
    // Not println!: a closed stdout (launcher gone) must not panic the runner.
    write_stdout(&format!(
        "frames={} checksum={:016x} nonblack={nonblack} loaded={loaded}\n",
        video.checksum.frames, video.checksum.last_checksum
    ));
    let _guard = guard.with_code(0);
    Ok(())
}
