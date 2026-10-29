//! Save-state files in the game's folder, per [`StateSlotId`] (`slotN` or the `auto`
//! resume point): `<stem>.state` (raw `retro_serialize` data) plus a `<stem>.json`
//! sidecar identifying the core that wrote it (and the SRAM / patch CRCs the
//! launcher's resume warnings use), and an optional `<stem>.png` thumbnail
//! (`state_thumbnail_writer`; presence = thumbnail).
//!
//! Load policy (`state_file_guard`, all before the core sees a byte): different core
//! name → reject (state format is core-specific); different core version → warn and
//! try (states usually survive updates), reject for strict-version cores; another
//! disc / revision (content tag) → reject; length / CRC differ → "damaged". A core
//! without states this session (no hardware context, `states_available`) → refused
//! before any file is read. Hardcore (`HardcoreGate`) refuses every load first: this
//! is the one chokepoint of F4, the menu, `--load-state` and the resume point.

use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context};
use config_layers::write_atomically;
// File names and the sidecar format are shared with the launcher (save-state list).
use ipc_protocol::{state_file, SIDECAR_FILE_EXT, STATE_FILE_EXT, THUMBNAIL_FILE_EXT};
pub use ipc_protocol::{StateSidecar, StateSlotId};
use libretro_host::{CoreSession, HostError};

use crate::hardcore_gate::{HardcoreGate, HardcoreLoadRefused};
use crate::runner_data_paths::RunnerDataPaths;
use crate::sram_persistence::sram_crc32;
use crate::state_file_guard::StateFileGuard;
use crate::state_load_checks::{check_state_size, unserialize_or_restore};

#[path = "state_failure_status.rs"]
mod state_failure_status;
pub use state_failure_status::{failure_status, STATES_NEED_HW_CONTEXT};

pub struct SaveStateSlots {
    dir: PathBuf,
    /// Largest state file read into memory (the core's `max_state_bytes`).
    max_state_bytes: u64,
    /// CRC of the soft-patch file found at start (sidecar `rom_patch_crc32`).
    rom_patch_crc32: Option<u32>,
    guard: StateFileGuard,
    /// Achievement hardcore: loads refused while active (saving stays allowed).
    hardcore: HardcoreGate,
}

impl SaveStateSlots {
    /// Lenient, untagged slots (tests; the session uses [`Self::for_game`]).
    pub fn new(dir: PathBuf, max_state_bytes: u64) -> Self {
        Self {
            dir,
            max_state_bytes,
            rom_patch_crc32: None,
            guard: StateFileGuard::default(),
            hardcore: HardcoreGate::default(),
        }
    }

    /// The game's slots with its core's state cap and guard (strict version, tag).
    pub fn for_game(paths: &RunnerDataPaths) -> Self {
        let max = crate::runner_core_options::max_state_bytes(paths.system);
        Self {
            guard: StateFileGuard::new(paths.system, paths.state_content_tag.clone()),
            ..Self::new(paths.states_dir.clone(), max)
        }
    }

    /// Records the session's soft-patch file CRC in every sidecar written from now on.
    pub fn set_rom_patch_crc32(&mut self, crc: Option<u32>) {
        self.rom_patch_crc32 = crc;
    }

    /// Shares the achievement session's hardcore state (loads refused while active).
    pub fn set_hardcore_gate(&mut self, gate: HardcoreGate) {
        self.hardcore = gate;
    }

    /// Whether loads are refused now (achievement hardcore).
    pub fn hardcore_active(&self) -> bool {
        self.hardcore.active()
    }

    /// (state file, sidecar file) for `id`.
    pub fn paths(&self, id: StateSlotId) -> (PathBuf, PathBuf) {
        (
            state_file(&self.dir, id, STATE_FILE_EXT),
            state_file(&self.dir, id, SIDECAR_FILE_EXT),
        )
    }

    /// Thumbnail file for `id` (written after the state, may be missing).
    pub fn thumbnail_path(&self, id: StateSlotId) -> PathBuf {
        state_file(&self.dir, id, THUMBNAIL_FILE_EXT)
    }

    /// `created_at` of the slot's sidecar, when the slot holds a state.
    pub fn saved_at(&self, id: StateSlotId) -> Option<u64> {
        let (state, sidecar) = self.paths(id);
        if !state.exists() {
            return None;
        }
        let raw = std::fs::read(sidecar).ok()?;
        serde_json::from_slice::<StateSidecar>(&raw)
            .ok()
            .map(|sidecar| sidecar.created_at)
    }

    /// Writes `id`; `progress` = the achievement session's hex progress (sidecar).
    pub fn save(
        &self,
        session: &mut CoreSession,
        id: StateSlotId,
        progress: Option<String>,
    ) -> anyhow::Result<()> {
        let started = Instant::now();
        let state = session.serialize()?;
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut sidecar = StateSidecar {
            core_name: session.core_info().name.clone(),
            core_version: session.core_info().version.clone(),
            created_at,
            sram_crc32: sram_crc32(session),
            rom_patch_crc32: self.rom_patch_crc32,
            achievements_progress: progress,
            ..StateSidecar::default()
        };
        self.guard.stamp(&mut sidecar, &state);
        let (state_path, sidecar_path) = self.paths(id);
        // Sidecar first: a crash in between never leaves a state without metadata;
        // old state + new sidecar then fails the length / CRC check ("damaged"),
        // refused before the core.
        write_atomically(&sidecar_path, &serde_json::to_vec_pretty(&sidecar)?)?;
        write_atomically(&state_path, &state)?;
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        tracing::info!(slot = %id, bytes = state.len(), ms, "state saved");
        Ok(())
    }

    /// Loads `id`; returns the sidecar's achievement progress (`None`: the state was
    /// saved without one, so achievement progress resets).
    pub fn load(
        &self,
        session: &mut CoreSession,
        id: StateSlotId,
    ) -> anyhow::Result<Option<String>> {
        if self.hardcore_active() {
            return Err(HardcoreLoadRefused.into()); // no file read, no core call
        }
        let (state_path, sidecar_path) = self.paths(id);
        if !state_path.exists() {
            bail!("slot {id} is empty");
        }
        if !session.states_available() {
            return Err(HostError::StatesNeedHwContext.into()); // no file read, no core call
        }
        let raw = std::fs::read(&sidecar_path)
            .with_context(|| format!("missing state metadata {}", sidecar_path.display()))?;
        let sidecar: StateSidecar =
            serde_json::from_slice(&raw).context("corrupt state metadata")?;
        self.guard.check_sidecar(&sidecar, session.core_info())?;
        let len = std::fs::metadata(&state_path)?.len();
        check_state_size(id, len, self.max_state_bytes)?;
        StateFileGuard::check_len(&sidecar, len)?;
        let state = std::fs::read(&state_path)?;
        StateFileGuard::check_crc(&sidecar, &state)?;
        unserialize_or_restore(session, &state)
            .with_context(|| format!("core rejected state in slot {id}"))?;
        tracing::info!(slot = %id, "state loaded");
        Ok(sidecar.achievements_progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sidecar(name: &str, version: &str) -> StateSidecar {
        StateSidecar {
            core_name: name.into(),
            core_version: version.into(),
            created_at: 1,
            ..StateSidecar::default()
        }
    }

    #[test]
    fn malformed_sidecar_fails_to_parse() {
        assert!(serde_json::from_slice::<StateSidecar>(b"{\"core_name\": 5}").is_err());
        let json = serde_json::to_vec(&sidecar("Snes9x", "1")).unwrap();
        assert_eq!(
            serde_json::from_slice::<StateSidecar>(&json).unwrap(),
            sidecar("Snes9x", "1")
        );
    }
}
