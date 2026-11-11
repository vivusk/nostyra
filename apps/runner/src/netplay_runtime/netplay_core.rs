//! What the netplay runtime needs from the emulator, as a trait so the lobby, sync
//! and rollback logic run against a fake core in tests and against the real
//! `CoreSession` (plus the loop's audio / save / achievement hooks) in the window
//! loop (`emulation_main_loop::netplay_glue`).

use libretro_host::AvEnable;
use netplay_session::netplay_input::{NetplayInput, NETPLAY_PORTS};
use netplay_session::sync_transfer::SyncPayload;

/// Emulator side of one netplay peer (core thread).
pub(crate) trait NetplayCore {
    /// Rollback save (`RunaheadSameInstance`) into `buf` (reused).
    fn save_into(&mut self, buf: &mut Vec<u8>) -> Result<(), String>;
    /// Restores a rollback save.
    fn load_from(&mut self, state: &[u8]) -> Result<(), String>;
    /// crc32 over system RAM + SRAM (`desync_checksum`), widened.
    fn ram_checksum(&mut self) -> u128;
    /// Runs one frame with these port inputs and AV switches (restores `ALL` after).
    fn run(&mut self, inputs: &[NetplayInput; NETPLAY_PORTS], av: AvEnable) -> Result<(), String>;
    /// Forward-path hooks of a newly advanced frame (achievements, SRAM flush, F20).
    fn after_new_frame(&mut self);
    /// Presentation hooks of the tick's last frame (audio DRC, turbo clock).
    fn after_presented_frame(&mut self);
    /// Whether the core can serialize yet (`serialize_size() > 0`).
    fn state_ready(&mut self) -> bool;
    /// Host: SRAM + RTC + a `Normal` full state for the guests.
    fn sync_payload(&mut self) -> Result<SyncPayload, String>;
    /// Guest: the host's SRAM / RTC copied in, then its `Normal` state loaded.
    fn load_sync(&mut self, payload: &SyncPayload) -> Result<(), String>;
    /// (SRAM length, RTC length) the host's transfer must match (F18).
    fn memory_sizes(&mut self) -> (u32, Option<u32>);
}

/// `MUST_INITIALIZE` warm-up cap (frames) before netplay is refused.
pub(crate) const MAX_WARMUP_FRAMES: u32 = 300;

/// Runs default-input frames with no AV until the core serializes; the count goes
/// into the fingerprint (0 for cores that serialize at once). `None` past the cap.
pub(crate) fn run_warmup(core: &mut dyn NetplayCore) -> Option<u32> {
    let idle = [NetplayInput::default(); NETPLAY_PORTS];
    for frames in 0..=MAX_WARMUP_FRAMES {
        if core.state_ready() {
            return Some(frames);
        }
        if frames == MAX_WARMUP_FRAMES || core.run(&idle, AvEnable::NONE).is_err() {
            break;
        }
    }
    None
}
