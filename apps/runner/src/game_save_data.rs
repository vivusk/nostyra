//! Bundles a game's persistent data (windowed loop and headless runs): SRAM and the
//! cartridge clock (when the core exposes one), state
//! slots with their thumbnails (numbered slots and the `auto` resume point), plus the
//! active slot (F2/F3, numbered only), the real frames run this session (auto-save
//! rule) and the status shown in the window title (the runner also flashes it on
//! screen, see `on_screen_status`).

use ipc_protocol::StateSlotId;
use libretro_host::CoreSession;
use video_renderer::RgbaFrameBuffer;

use crate::pre_boot_load_guard::{PreBootLoadGuard, CORE_NOT_READY};
use crate::rom_soft_patching::SoftPatchedRom;
use crate::runner_cli_args::MAX_STATE_SLOT;
use crate::runner_data_paths::RunnerDataPaths;
use crate::save_state_slots::{failure_status, SaveStateSlots};
use crate::sram_persistence::{BatteryMemory, SramPersistence};
use crate::state_thumbnail_writer::{StateThumbnailWriter, Thumbnail};

mod game_window_title;
mod hardcore_state_load;

/// [`GameSaveData::load_state`] status after a successful load.
pub const STATE_LOADED: &str = "Loaded";
/// [`GameSaveData::save_state`] status after a successful save.
pub const STATE_SAVED: &str = "Saved";

pub struct GameSaveData {
    sram: SramPersistence,
    /// `rtc.bin`: a no-op for games without a core RTC.
    rtc: SramPersistence,
    states: SaveStateSlots,
    /// Encodes `slotN.png` off the frame thread; joined when this is dropped.
    thumbnails: StateThumbnailWriter,
    rom_name: String,
    /// Shown after the ROM name, e.g. "IPS patch" (soft-patching).
    title_label: Option<String>,
    slot: u8,
    /// Real-timeline frames emulated this session (counted by [`Self::tick`]).
    real_frames: u64,
    boot: PreBootLoadGuard,
}

impl GameSaveData {
    /// Loads SRAM and the cartridge clock into the session; must run before the
    /// first frame.
    pub fn load(paths: &RunnerDataPaths, session: &mut CoreSession, slot: u8) -> Self {
        Self {
            sram: SramPersistence::load(BatteryMemory::SaveRam, paths.sram_file.clone(), session),
            rtc: SramPersistence::load(BatteryMemory::Rtc, paths.rtc_file.clone(), session),
            states: SaveStateSlots::for_game(paths),
            thumbnails: StateThumbnailWriter::start(),
            rom_name: paths.rom_name.clone(),
            title_label: None,
            slot: slot.min(MAX_STATE_SLOT),
            real_frames: 0,
            boot: PreBootLoadGuard::new(session),
        }
    }

    /// Soft-patch facts: the window-title label and the patch file CRC every state
    /// sidecar records.
    pub fn with_soft_patch(mut self, patched: &SoftPatchedRom) -> Self {
        self.title_label = patched.title_label.clone();
        self.states.set_rom_patch_crc32(patched.patch_crc32);
        self
    }

    /// Per-frame housekeeping (periodic SRAM + clock flush). Call once per real-timeline
    /// frame (normal speed and fast-forward; never run-ahead's hidden frames or rewind
    /// playback), so it also counts the frames the auto-save rule needs.
    pub fn tick(&mut self, session: &mut CoreSession) {
        self.real_frames += 1;
        self.sram.flush_if_due(session);
        self.rtc.flush_if_due(session);
    }

    /// Real-timeline frames emulated this session.
    pub fn real_frames(&self) -> u64 {
        self.real_frames
    }

    /// F1. Failures are logged and reported in the title, never fatal. `frame` is the
    /// clean core frame for the thumbnail. With run-ahead N it is the presented frame,
    /// N (≤ 2) frames ahead of the saved state: accepted, the offset is cosmetic.
    pub fn save_state(
        &self,
        session: &mut CoreSession,
        frame: &RgbaFrameBuffer,
        progress: Option<String>,
    ) -> String {
        match self.save_slot(session, StateSlotId::Numbered(self.slot), frame, progress) {
            Ok(()) => STATE_SAVED.to_string(),
            Err(err) => {
                tracing::warn!("save state failed: {err:#}");
                failure_status(&err, "Save failed")
            }
        }
    }

    /// Saves `id` (sidecar with the achievement `progress` + state), then queues its
    /// thumbnail from `frame`; errors returned as-is (F1 reports them, auto-save on
    /// exit logs them).
    pub fn save_slot(
        &self,
        session: &mut CoreSession,
        id: StateSlotId,
        frame: &RgbaFrameBuffer,
        progress: Option<String>,
    ) -> anyhow::Result<()> {
        self.states.save(session, id, progress)?;
        // After the state: a failed or missing thumbnail never fails the save.
        let path = self.states.thumbnail_path(id);
        self.thumbnails.queue(path, Thumbnail::from_frame(frame));
        Ok(())
    }

    /// F4: (status, the state's achievement progress). The progress is meaningful
    /// only when the status is [`STATE_LOADED`].
    pub fn load_state(&self, session: &mut CoreSession) -> (String, Option<String>) {
        if let Some(refused) = self.hardcore_refusal() {
            return (refused, None); // before the boot check (a core call)
        }
        let id = StateSlotId::Numbered(self.slot);
        if !self.has_state(id) {
            return ("Empty slot".to_string(), None);
        }
        if !self.boot.ready(session) {
            return (CORE_NOT_READY.to_string(), None);
        }
        match self.load_slot(session, id) {
            Ok(progress) => (STATE_LOADED.to_string(), progress),
            Err(err) => {
                tracing::warn!("load state failed: {err:#}");
                (failure_status(&err, "Load failed"), None)
            }
        }
    }

    /// Loads `id` with errors returned as-is (startup `--load-state` retries on them);
    /// `Ok` carries the sidecar's achievement progress.
    pub fn load_slot(
        &self,
        session: &mut CoreSession,
        id: StateSlotId,
    ) -> anyhow::Result<Option<String>> {
        self.states.load(session, id)
    }

    /// Whether `id` holds a state file.
    pub fn has_state(&self, id: StateSlotId) -> bool {
        self.states.paths(id).0.exists()
    }

    /// When numbered `slot` was saved (Unix seconds, from its sidecar), if it holds a
    /// state (slot grid; the resume point is not shown there).
    pub fn saved_at(&self, slot: u8) -> Option<u64> {
        self.states.saved_at(StateSlotId::Numbered(slot))
    }

    /// `slotN.png` (may be missing or still being written).
    pub fn thumbnail_path(&self, slot: u8) -> std::path::PathBuf {
        self.states.thumbnail_path(StateSlotId::Numbered(slot))
    }

    /// File-name-safe ROM name (pause-menu title).
    pub fn rom_name(&self) -> &str {
        &self.rom_name
    }

    /// F2 (-1) / F3 (+1), clamped to 0..=MAX_STATE_SLOT.
    pub fn change_slot(&mut self, delta: i8) -> String {
        self.slot = self.slot.saturating_add_signed(delta).min(MAX_STATE_SLOT);
        tracing::info!(slot = self.slot, "state slot selected");
        (if self.has_state(StateSlotId::Numbered(self.slot)) {
            "Has state"
        } else {
            "Empty"
        })
        .to_string()
    }

    /// Makes `slot` the active one (clamped), e.g. `--headless-save-slot`.
    pub fn select_slot(&mut self, slot: u8) {
        self.slot = slot.min(MAX_STATE_SLOT);
    }

    /// Active state slot (0..=MAX_STATE_SLOT).
    pub fn slot(&self) -> u8 {
        self.slot
    }

    /// On exit: final SRAM + clock write.
    pub fn flush(&mut self, session: &mut CoreSession) {
        self.sram.flush(session);
        self.rtc.flush(session);
    }
}
