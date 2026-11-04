//! Loop side of the periodic resume point (`periodic_resume_save`): checked after
//! each redraw's frames, saved with the clean core frame and the achievement
//! progress; a failure is only logged (the previous point stays intact).

use std::time::Instant;

use super::EmulationApp;

impl EmulationApp<'_> {
    /// Saves the `auto` slot in the background when its interval has passed.
    pub(super) fn save_periodic_resume_point(&mut self) {
        let allowed = self.config.resume.auto_save
            && self.session.states_available()
            && self.startup_load.is_none();
        let frames = self.saves.real_frames();
        if !self.periodic_resume.due(Instant::now(), frames, allowed) {
            return;
        }
        let progress = self.achievement_progress();
        let (saves, session, frame) = (&mut self.saves, &mut self.session, &self.frame);
        if let Err(err) = saves.save_resume_point_in_background(session, frame, progress) {
            tracing::warn!("periodic resume point failed: {err:#}");
        }
    }
}
