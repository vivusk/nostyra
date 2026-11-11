//! What reaches the disk: the persist switch (netplay guests and spectators write
//! no SRAM, clock or resume point, M20) and the periodic resume point written in
//! the background.

use ipc_protocol::StateSlotId;
use libretro_host::CoreSession;
use video_renderer::RgbaFrameBuffer;

use super::GameSaveData;
use crate::state_thumbnail_writer::Thumbnail;

impl GameSaveData {
    /// `false`: SRAM / clock flushes and resume points are skipped (netplay guest).
    pub fn set_persist(&mut self, persist: bool) {
        self.persist = persist;
    }

    /// Whether this session writes its save data.
    pub fn persists(&self) -> bool {
        self.persist
    }

    /// Periodic resume point: SRAM + clock flushed first (the sidecar's `sram_crc32`
    /// then matches the `.srm` on disk), the core serialized here, the files and the
    /// thumbnail written by the background worker (no disk wait on the frame thread).
    pub fn save_resume_point_in_background(
        &mut self,
        session: &mut CoreSession,
        frame: &RgbaFrameBuffer,
        progress: Option<String>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(self.persist, "netplay guest: nothing is saved");
        self.flush(session);
        let id = StateSlotId::Auto;
        let files = self.states.prepare(session, id, progress)?;
        tracing::info!(bytes = files.state_len(), "periodic resume point queued");
        self.thumbnails.queue_files(files)?;
        let path = self.states.thumbnail_path(id);
        self.thumbnails.queue(path, Thumbnail::from_frame(frame));
        Ok(())
    }

    /// Waits for queued background writes (before the exit save overwrites them).
    pub fn wait_background_writes(&self) {
        self.thumbnails.wait_idle();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code

    use std::sync::PoisonError;

    use libretro_host::{NullAudioSink, NullInputSource, NullVideoSink};

    use super::*;
    use crate::real_core_session_loader::load_session;
    use crate::runner_data_paths::RunnerDataPaths;
    use crate::runner_real_core_suite::{repo, scratch, CORE_LOCK};

    /// A netplay guest's 600 frames (and the exit flush) write no file (snes9x).
    #[test]
    fn guest_writes_nothing_to_its_save_folder() {
        let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let rom = repo("test-assets/roms/nostyra-test.sfc");
        let data = scratch("netplay-guest-saves");
        let mut session = load_session("snes9x", &rom, data.path());
        let paths =
            RunnerDataPaths::resolve(Some(data.path()), &rom, &system_catalog::SNES, Some("ab12"))
                .unwrap();
        let mut saves = GameSaveData::load(&paths, &mut session, 0);
        saves.set_persist(false);
        let (mut video, mut audio, mut input) = (
            NullVideoSink::default(),
            NullAudioSink::default(),
            NullInputSource,
        );
        for _ in 0..600 {
            session
                .run_frame(&mut video, &mut audio, &mut input)
                .unwrap();
            saves.tick(&mut session);
        }
        saves.flush(&mut session);
        let frame = RgbaFrameBuffer::default();
        assert!(saves
            .save_resume_point_in_background(&mut session, &frame, None)
            .is_err());
        saves.wait_background_writes();
        let files = std::fs::read_dir(&paths.saves_dir).map_or(0, |dir| dir.count());
        assert_eq!(files, 0, "guest save folder stays empty");
        assert_eq!(saves.real_frames(), 600, "frames still counted");
    }
}
