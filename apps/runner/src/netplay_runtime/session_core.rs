//! [`NetplayCore`] on the real `CoreSession`: rollback states in the
//! `RunaheadSameInstance` context, inputs through a `NetplayInputSource`, audio to
//! the loop's sink (or muted), and the loop's hooks after frames
//! ([`NetplayFrameHooks`]: audio DRC / turbo clock, achievements / SRAM flush).

use libretro_host::{
    AudioSink, AvEnable, CoreSession, InputSource, NullAudioSink, SavestateContext, VideoSink,
};
use netplay_session::netplay_input::{NetplayInput, NETPLAY_PORTS};
use netplay_session::sync_transfer::SyncPayload;

use super::netplay_core::NetplayCore;
use super::save_sync;
use input_mapping::NetplayInputSource;

const ROLLBACK: SavestateContext = SavestateContext::RunaheadSameInstance;

/// What the window loop does around netplay frames.
pub(crate) trait NetplayFrameHooks {
    /// Sink for frames whose audio is played.
    fn audio_sink(&mut self) -> &mut (dyn AudioSink + 'static);
    /// Forward path (F20): achievements `do_frame`, SRAM flush tick.
    fn after_new_frame(&mut self, session: &mut CoreSession);
    /// Last frame of the tick: audio DRC update, turbo clock.
    fn after_presented_frame(&mut self);
}

/// Hooks of the load-time warm-up and the synctest: nothing heard, nothing hooked.
#[derive(Default)]
pub(crate) struct SilentHooks(NullAudioSink);

impl NetplayFrameHooks for SilentHooks {
    fn audio_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        &mut self.0
    }
    fn after_new_frame(&mut self, _session: &mut CoreSession) {}
    fn after_presented_frame(&mut self) {}
}

/// One borrow of the loop's core, picture, netplay input table and hooks.
pub(crate) struct SessionCore<'a> {
    pub session: &'a mut CoreSession,
    pub video: &'a mut (dyn VideoSink + 'static),
    pub input: &'a mut NetplayInputSource,
    /// The system has analog sticks (axes kept).
    pub analog: bool,
    pub hooks: &'a mut dyn NetplayFrameHooks,
}

impl NetplayCore for SessionCore<'_> {
    fn save_into(&mut self, buf: &mut Vec<u8>) -> Result<(), String> {
        let saved = self.session.serialize_into(buf, ROLLBACK);
        saved.map(|_| ()).map_err(|err| err.to_string())
    }

    fn load_from(&mut self, state: &[u8]) -> Result<(), String> {
        let loaded = self.session.unserialize_from(state, ROLLBACK);
        loaded.map_err(|err| err.to_string())
    }

    fn ram_checksum(&mut self) -> u128 {
        // Same bytes and order as `desync_checksum(system_ram, sram)`.
        let mut hasher = crc32fast::Hasher::new();
        if let Some(ram) = self.session.system_ram() {
            hasher.update(ram);
        }
        if let Some(sram) = self.session.save_ram_mut() {
            hasher.update(sram);
        }
        u128::from(hasher.finalize())
    }

    fn run(&mut self, inputs: &[NetplayInput; NETPLAY_PORTS], av: AvEnable) -> Result<(), String> {
        self.input.set_frame_inputs(inputs, self.analog);
        let mut muted = NullAudioSink::default();
        let audio: &mut (dyn AudioSink + 'static) = match av.audio {
            true => self.hooks.audio_sink(),
            false => &mut muted,
        };
        self.session.set_av_enable(av);
        let input: &mut (dyn InputSource + 'static) = self.input;
        let ran = self.session.run_frame(self.video, audio, input);
        self.session.set_av_enable(AvEnable::ALL);
        ran.map_err(|err| err.to_string())
    }

    fn after_new_frame(&mut self) {
        self.hooks.after_new_frame(self.session);
    }

    fn after_presented_frame(&mut self) {
        self.hooks.after_presented_frame();
    }

    fn state_ready(&mut self) -> bool {
        self.session.current_state_size() > 0
    }

    fn sync_payload(&mut self) -> Result<SyncPayload, String> {
        save_sync::host_payload(self.session)
    }

    fn load_sync(&mut self, payload: &SyncPayload) -> Result<(), String> {
        save_sync::guest_load(self.session, payload)
    }

    fn memory_sizes(&mut self) -> (u32, Option<u32>) {
        let sram = self.session.save_ram_mut().map_or(0, |m| m.len() as u32);
        (sram, self.session.rtc_mut().map(|m| m.len() as u32))
    }
}
