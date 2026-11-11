//! The runner's `RollbackHost`: ggrs requests on a [`NetplayCore`]. `save` =
//! rollback serialize + desync checksum (also the frames-limit crc history),
//! `load` = unserialize, `advance` = the frame's inputs on ports 0..n, AV per plan
//! "Audio on re-sim" (re-simulation → nothing, new frame inside a multi-frame tick
//! → audio only, the tick's last frame → video + audio of a new frame), then the
//! presentation hooks (last frame only) and the forward-path hooks (new frames only,
//! F20). Never rewind captures, never run-ahead.

use std::sync::atomic::{AtomicBool, Ordering};

use ggrs::{Frame, InputStatus};
use libretro_host::AvEnable;
use netplay_session::netplay_input::{NetplayInput, NETPLAY_PORTS};
use netplay_session::rollback_driver::RollbackHost;
use netplay_session::NetplayError;

use super::netplay_core::NetplayCore;
use super::ram_crc_history::FrameCrcs;

const AUDIO_ONLY: AvEnable = AvEnable {
    video: false,
    audio: true,
};

/// AV switches of an advanced frame.
pub(crate) fn av_for(is_new: bool, last_of_tick: bool) -> AvEnable {
    match (is_new, last_of_tick) {
        (_, true) => AvEnable {
            video: true,
            audio: is_new,
        },
        (true, false) => AUDIO_ONLY,
        (false, false) => AvEnable::NONE,
    }
}

/// One tick's borrow of the core and the crc history.
pub(crate) struct CoreRollbackHost<'a> {
    pub core: &'a mut dyn NetplayCore,
    pub ram_crc: &'a mut FrameCrcs,
    /// Spectators get no saves: they record the crc after each advance (F34).
    pub spectator: bool,
}

fn host_error(text: String) -> NetplayError {
    NetplayError::Host(text)
}

impl RollbackHost for CoreRollbackHost<'_> {
    fn save(&mut self, frame: Frame, buf: &mut Vec<u8>) -> Result<u128, NetplayError> {
        self.core.save_into(buf).map_err(host_error)?;
        let checksum = self.core.ram_checksum();
        self.ram_crc.ram.record(frame, checksum as u32);
        Ok(checksum)
    }

    fn load(&mut self, _frame: Frame, state: &[u8]) -> Result<(), NetplayError> {
        self.core.load_from(state).map_err(host_error)
    }

    fn advance(
        &mut self,
        frame: Frame,
        inputs: &[(NetplayInput, InputStatus)],
        is_new: bool,
        last_of_tick: bool,
    ) {
        let mut ports = [NetplayInput::default(); NETPLAY_PORTS];
        let mut bytes = [0u8; NETPLAY_PORTS * 10];
        for (i, (port, (input, _))) in ports.iter_mut().zip(inputs).enumerate() {
            *port = *input;
            bytes[i * 10..i * 10 + 2].copy_from_slice(&input.buttons.to_le_bytes());
            for (a, axis) in input.axes.iter().enumerate() {
                bytes[i * 10 + 2 + a * 2..i * 10 + 4 + a * 2].copy_from_slice(&axis.to_le_bytes());
            }
        }
        self.ram_crc.record_inputs(frame, &bytes);
        if let Err(err) = self.core.run(&ports, av_for(is_new, last_of_tick)) {
            // A core error mid-frame: the desync check reports any divergence.
            // Logged once per runner (re-simulated frames would flood the log).
            static LOGGED: AtomicBool = AtomicBool::new(false);
            if !LOGGED.swap(true, Ordering::Relaxed) {
                tracing::warn!("netplay frame failed: {err}");
            }
        }
        if last_of_tick {
            self.core.after_presented_frame();
        }
        if is_new {
            self.core.after_new_frame();
        }
        if self.spectator {
            // State at the start of the next frame (= the players' save of it).
            let checksum = self.core.ram_checksum();
            self.ram_crc.ram.record(frame + 1, checksum as u32);
        }
    }
}

#[cfg(test)]
#[path = "rollback_host_tests.rs"]
mod tests;
