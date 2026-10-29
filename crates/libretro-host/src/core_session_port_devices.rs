//! Controller port devices (`retro_set_controller_port_device`).
//!
//! Most cores default every port to a RetroPad and never need the call; some do:
//! Flycast connects no controller (and no VMU) on any port until each of its four
//! ports has been set. The call is opt-in per core (the runner passes the core's
//! port list), so cores that change behaviour on it (e.g. PCSX ReARMed switching pad
//! types) are never touched.

use std::ffi::c_uint;

use crate::core_session::CoreSession;

impl CoreSession {
    /// Tells the core which device sits on each port: `devices[i]` goes to port `i`
    /// (`RETRO_DEVICE_*`). Call after `CoreSession::load` (= after `retro_load_game`)
    /// and before the first frame. An empty slice makes no core call; a core without
    /// the export is skipped with a warning.
    pub fn set_controller_port_devices(&mut self, devices: &[u32]) {
        if devices.is_empty() {
            return;
        }
        let Some(set_device) = self.core.set_controller_port_device else {
            tracing::warn!("core has no retro_set_controller_port_device; ports left as is");
            return;
        };
        self.make_hw_current();
        for (port, &device) in devices.iter().enumerate() {
            // SAFETY: game loaded; plain call on the session thread, no context borrow
            // is held across it.
            unsafe { set_device(port as c_uint, device) };
        }
        tracing::debug!(?devices, "controller port devices set");
    }
}

#[cfg(test)]
#[path = "core_session_port_devices_tests.rs"]
mod tests;
