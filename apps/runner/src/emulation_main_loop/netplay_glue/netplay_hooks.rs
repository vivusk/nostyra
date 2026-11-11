//! The window loop's hooks around netplay frames: played audio goes to the loop's
//! sink; the tick's last frame updates the audio DRC and the turbo clock; every
//! newly advanced frame (never a re-simulation, F20) feeds achievements and the
//! SRAM flush tick. No rewind capture, no run-ahead.

use std::cell::Cell;

use libretro_host::{AudioSink, CoreSession};

use crate::achievement_session::{self, AchievementSession};
use crate::game_save_data::GameSaveData;
use crate::loop_timing::LoopTiming;
use crate::netplay_runtime::session_core::NetplayFrameHooks;

pub(in crate::emulation_main_loop) struct NetplayLoopHooks<'a> {
    pub timing: &'a mut LoopTiming,
    pub saves: &'a mut GameSaveData,
    pub achievements: &'a mut Option<AchievementSession>,
    pub turbo_clock: &'a Cell<u64>,
}

impl NetplayFrameHooks for NetplayLoopHooks<'_> {
    fn audio_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        self.timing.audio_sink()
    }

    fn after_new_frame(&mut self, session: &mut CoreSession) {
        self.saves.tick(session); // counts the frame; flushes only when persisting
        achievement_session::after_real_frame(self.achievements, session);
    }

    fn after_presented_frame(&mut self) {
        self.timing.after_frame();
        self.turbo_clock.set(self.turbo_clock.get().wrapping_add(1));
    }
}
