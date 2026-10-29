//! Save data access on a running session: soft reset, save states, battery SRAM and
//! cartridge clock (RTC);
//! plus the audio/video enable switch used by fast-forward and run-ahead.
//!
//! Save states go through one path (`serialize_into` / `unserialize_from`): the
//! size is cached for fixed-size cores, the caller's buffer is reused (no per-frame
//! allocation for run-ahead/rewind) and the core is told the savestate context.

use crate::core_session::CoreSession;
use crate::ffi_types::{RETRO_MEMORY_RTC, RETRO_MEMORY_SAVE_RAM, RETRO_MEMORY_SYSTEM_RAM};
use crate::host_context::AvEnable;
use crate::host_error::{HostError, HostResult};
use crate::savestate_context::SavestateContext;
use crate::serialization_quirks::{resolve_state_size, SerializationQuirks};

impl CoreSession {
    /// Outputs the core should produce from the next `run_frame` on. Disabled ones are
    /// skipped by cores that honour `GET_AUDIO_VIDEO_ENABLE` (e.g. snes9x) and
    /// discarded by the host otherwise. Emulation state is unaffected.
    pub fn set_av_enable(&mut self, av: AvEnable) {
        self.ctx_mut().av_enable = av;
    }

    /// Frame skip for fast-forward: no video, no audio while on.
    pub fn set_frame_skip(&mut self, skip: bool) {
        self.set_av_enable(if skip { AvEnable::NONE } else { AvEnable::ALL });
    }

    /// Quirks the core reported via `SET_SERIALIZATION_QUIRKS` (none if it never did).
    pub fn serialization_quirks(&self) -> SerializationQuirks {
        self.ctx().serialization_quirks
    }

    /// Soft reset (`retro_reset`).
    pub fn reset(&mut self) {
        self.make_hw_current();
        // SAFETY: game loaded; no context reference held.
        unsafe { (self.core.reset)() };
    }

    /// Creates a save state for a slot file (`Normal` context).
    pub fn serialize(&mut self) -> HostResult<Vec<u8>> {
        let mut buf = Vec::new();
        self.serialize_into(&mut buf, SavestateContext::Normal)?;
        Ok(buf)
    }

    /// Restores a save state produced by [`Self::serialize`].
    pub fn unserialize(&mut self, state: &[u8]) -> HostResult<()> {
        self.unserialize_from(state, SavestateContext::Normal)
    }

    /// Writes a save state into `buf` (resized to the state size, capacity reused) and
    /// returns its length. `SerializeNotReady`: a `MUST_INITIALIZE` core is not ready
    /// yet; retry on a later frame.
    pub fn serialize_into(
        &mut self,
        buf: &mut Vec<u8>,
        context: SavestateContext,
    ) -> HostResult<usize> {
        self.ensure_states_available()?;
        let quirks = self.serialization_quirks();
        self.make_hw_current(); // cores may render while they reach a safe point
        let size_quirks = self.size_quirks();
        let core = &self.core;
        // SAFETY: plain query; game loaded.
        let size = resolve_state_size(&mut self.state_size, size_quirks, || unsafe {
            (core.serialize_size)()
        })?;
        buf.resize(size, 0);
        let serialize = self.core.serialize;
        // SAFETY: `buf` holds exactly `size` writable bytes.
        let ok = self.with_context(context, || unsafe {
            serialize(buf.as_mut_ptr().cast(), size)
        });
        match ok {
            true => Ok(size),
            false if quirks.must_initialize() => Err(HostError::SerializeNotReady),
            false => Err(HostError::SerializeFailed),
        }
    }

    /// Refuse, without calling the core, any state whose length differs from the
    /// core's fixed state size (when the size is known: not for variable-size cores,
    /// nor before a `MUST_INITIALIZE` core reports one). For cores that cannot reject
    /// a bad state themselves: Mupen64Plus-Next's `retro_unserialize` waits for a load
    /// that never finishes (the caller hangs). Off by default (older-version states of
    /// other cores may differ in size and still load).
    pub fn set_exact_state_size(&mut self, exact: bool) {
        self.exact_state_size = exact;
    }

    /// Never cache the state size: query `retro_serialize_size` before every serialize,
    /// as for a `CORE_VARIABLE_SIZE` core. For cores whose size changes without saying
    /// so (PPSSPP: 128 MiB before boot, then measured per call). Off by default.
    pub fn set_state_size_requery(&mut self, requery: bool) {
        self.ctx_mut().state_size_requery = requery;
    }

    /// Whether [`Self::set_state_size_requery`] is on.
    pub fn state_size_requery(&self) -> bool {
        self.ctx().state_size_requery
    }

    /// The core's state size right now (`retro_serialize_size`, never cached); e.g. to
    /// see a re-query core finish booting. Some cores render to reach a safe point.
    /// 0 while states are unavailable (no core call).
    pub fn current_state_size(&mut self) -> usize {
        if !self.states_available() {
            return 0;
        }
        self.make_hw_current();
        // SAFETY: plain query; game loaded.
        unsafe { (self.core.serialize_size)() }
    }

    /// Quirks the size rule uses: the core's, variable when re-query is on.
    fn size_quirks(&self) -> SerializationQuirks {
        let quirks = self.serialization_quirks();
        match self.state_size_requery() {
            true => quirks.with_variable_size(),
            false => quirks,
        }
    }

    /// Restores a state from `state` under the given context.
    pub fn unserialize_from(&mut self, state: &[u8], context: SavestateContext) -> HostResult<()> {
        self.ensure_states_available()?;
        if self.exact_state_size {
            let quirks = self.size_quirks();
            let core = &self.core;
            // SAFETY: plain query; game loaded.
            let size = resolve_state_size(&mut self.state_size, quirks, || unsafe {
                (core.serialize_size)()
            });
            match size {
                Ok(size) if size != state.len() => return Err(HostError::UnserializeFailed),
                // Size 0: this game has no states (melonDS DS on a DSi title).
                Err(HostError::SerializeUnsupported) => {
                    return Err(HostError::SerializeUnsupported)
                }
                _ => {}
            }
        }
        self.make_hw_current();
        let unserialize = self.core.unserialize;
        // SAFETY: core only reads `state.len()` bytes.
        let ok = self.with_context(context, || unsafe {
            unserialize(state.as_ptr().cast(), state.len())
        });
        ok.then_some(()).ok_or(HostError::UnserializeFailed)
    }

    /// Runs one serialize/unserialize FFI call with `context` answered to the core
    /// (`GET_SAVESTATE_CONTEXT`), then back to `Normal`.
    fn with_context(&mut self, context: SavestateContext, call: impl FnOnce() -> bool) -> bool {
        self.ctx_mut().savestate_context = context;
        let ok = call(); // no context reference is held across the FFI call
        self.ctx_mut().savestate_context = SavestateContext::Normal;
        ok
    }

    /// Battery-backed cartridge RAM (`.srm` contents), if the game has any.
    /// Mutable so the runner can load a saved `.srm` into it before the first frame.
    pub fn save_ram_mut(&mut self) -> Option<&mut [u8]> {
        self.memory_mut(RETRO_MEMORY_SAVE_RAM)
    }

    /// Cartridge real-time clock (e.g. mGBA's Game Boy MBC3 clock, kept apart from
    /// SRAM), if the game has one. Persisted beside the `.srm` so in-game clocks keep
    /// running across restarts; loaded before the first frame like SRAM.
    pub fn rtc_mut(&mut self) -> Option<&mut [u8]> {
        self.memory_mut(RETRO_MEMORY_RTC)
    }

    /// The console's main RAM (`RETRO_MEMORY_SYSTEM_RAM`; Dolphin: MEM1, 24 MiB), if
    /// the core exposes it. Read-only; real-core tests inspect test-program state.
    pub fn system_ram(&mut self) -> Option<&[u8]> {
        self.memory_mut(RETRO_MEMORY_SYSTEM_RAM).map(|ram| &*ram)
    }
}
