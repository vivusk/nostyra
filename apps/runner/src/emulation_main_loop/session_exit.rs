//! End of a windowed session: a fatal error stops the loop (`fail`); after the loop
//! `finish` writes the resume point (with achievement progress), the final SRAM and
//! ends the achievement session before the core unloads.

use ipc_protocol::StateSlotId;
use winit::event_loop::ActiveEventLoop;

use super::EmulationApp;
use crate::auto_save_on_exit::{auto_save_on_exit, ExitFacts};

impl EmulationApp<'_> {
    /// After the event loop (every clean exit, and after a loop error): resume point
    /// (`auto_save_on_exit` policy; never affects the result), final SRAM write, then
    /// the fatal error (if any). The state is the real timeline: run-ahead has rolled
    /// back, a rewind shows its restored state, the pause menu only pauses.
    /// Leaves the session in `self`: the caller drops it after arming the teardown
    /// watchdog (`loaded_session_run`).
    pub fn finish(&mut self) -> anyhow::Result<()> {
        let facts = ExitFacts {
            enabled: self.config.resume.auto_save,
            states_available: self.session.states_available(),
            started: self.presenter.is_some(),
            loop_failed: self.error.is_some(),
            real_frames: self.saves.real_frames(),
        };
        let progress = self.achievement_progress();
        let (saves, session, frame) = (&self.saves, &mut self.session, &self.frame);
        // `frame` = clean last core frame (no OSD/shader/menu) for the thumbnail.
        auto_save_on_exit(facts, || {
            saves.save_slot(session, StateSlotId::Auto, frame, progress)
        });
        self.saves.flush(&mut self.session);
        self.shutdown_achievements();
        self.error.take().map_or(Ok(()), Err)
    }

    pub(super) fn fail(&mut self, event_loop: &ActiveEventLoop, err: anyhow::Error) {
        tracing::error!("{err:#}");
        self.error.get_or_insert(err);
        event_loop.exit();
    }
}
