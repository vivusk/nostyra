//! Test double of [`NetplayCore`]: a deterministic "console" whose RAM mixes every
//! port's input into a running hash each frame (so remote input changes RAM), with
//! an optional `MUST_INITIALIZE` warm-up and a call log for order assertions.

use libretro_host::AvEnable;
use netplay_session::desync_checksum::desync_checksum;
use netplay_session::netplay_input::{NetplayInput, NETPLAY_PORTS};
use netplay_session::sync_transfer::SyncPayload;

use super::netplay_core::NetplayCore;

#[derive(Debug, Default, Clone)]
pub(crate) struct FakeNetplayCore {
    pub ram: [u8; 32],
    pub sram: Vec<u8>,
    pub frames_run: u32,
    /// Serialize fails until this many frames ran (`MUST_INITIALIZE`).
    pub warmup: u32,
    pub log: Vec<String>,
    pub avs: Vec<AvEnable>,
    pub new_frame_hooks: u32,
    pub presented_hooks: u32,
}

impl FakeNetplayCore {
    pub(crate) fn new(warmup: u32) -> Self {
        Self {
            sram: vec![0; 8],
            warmup,
            ..Self::default()
        }
    }

    fn state(&self) -> Vec<u8> {
        let mut state = self.ram.to_vec();
        state.extend_from_slice(&self.frames_run.to_le_bytes());
        state
    }

    fn restore(&mut self, state: &[u8]) -> Result<(), String> {
        if state.len() != 36 {
            return Err(format!("bad state length {}", state.len()));
        }
        self.ram.copy_from_slice(&state[..32]);
        self.frames_run = u32::from_le_bytes([state[32], state[33], state[34], state[35]]);
        Ok(())
    }
}

impl NetplayCore for FakeNetplayCore {
    fn save_into(&mut self, buf: &mut Vec<u8>) -> Result<(), String> {
        self.log.push("save".into());
        buf.clear();
        buf.extend_from_slice(&self.state());
        Ok(())
    }

    fn load_from(&mut self, state: &[u8]) -> Result<(), String> {
        self.log.push("load".into());
        self.restore(state)
    }

    fn ram_checksum(&mut self) -> u128 {
        desync_checksum(&self.ram, &self.sram)
    }

    fn run(&mut self, inputs: &[NetplayInput; NETPLAY_PORTS], av: AvEnable) -> Result<(), String> {
        self.log.push("run".into());
        self.avs.push(av);
        for (port, input) in inputs.iter().enumerate() {
            let slot = &mut self.ram[port * 8..port * 8 + 8];
            let mixed = u64::from_le_bytes(slot.try_into().unwrap_or_default())
                .wrapping_mul(31)
                .wrapping_add(u64::from(input.buttons) + 1);
            slot.copy_from_slice(&mixed.to_le_bytes());
        }
        self.frames_run += 1;
        Ok(())
    }

    fn after_new_frame(&mut self) {
        self.new_frame_hooks += 1;
    }

    fn after_presented_frame(&mut self) {
        self.presented_hooks += 1;
    }

    fn state_ready(&mut self) -> bool {
        self.frames_run >= self.warmup
    }

    fn sync_payload(&mut self) -> Result<SyncPayload, String> {
        Ok(SyncPayload {
            sram: self.sram.clone(),
            rtc: None,
            state: self.state(),
        })
    }

    fn load_sync(&mut self, payload: &SyncPayload) -> Result<(), String> {
        self.sram.copy_from_slice(&payload.sram);
        self.restore(&payload.state)
    }

    fn memory_sizes(&mut self) -> (u32, Option<u32>) {
        (self.sram.len() as u32, None)
    }
}
