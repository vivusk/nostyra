//! Status text for a failed slot save/load (F1/F4, pause menu, `--load-state`).

use libretro_host::HostError;

use crate::hardcore_gate::{HardcoreLoadRefused, STATE_LOAD_REFUSED};

/// F1/F4 status when the game has no save states (the core's state size is 0, e.g.
/// melonDS DS on a DSi title).
pub const STATES_UNAVAILABLE: &str = "Save states are not available for this game";

/// F1/F4 status when the core's states need its hardware context and the session
/// has none (Dolphin on a Null renderer: its state calls crash).
pub const STATES_NEED_HW_CONTEXT: &str = "Save states unavailable without hardware rendering";

/// Short status for a failed slot save/load: [`STATES_UNAVAILABLE`] when the core
/// has no states for this game, [`STATES_NEED_HW_CONTEXT`] without the context the
/// core's states need, [`STATE_LOAD_REFUSED`] in hardcore, else `fallback`.
pub fn failure_status(err: &anyhow::Error, fallback: &str) -> String {
    if err.downcast_ref::<HardcoreLoadRefused>().is_some() {
        return STATE_LOAD_REFUSED.to_string();
    }
    let host = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<HostError>());
    match host {
        Some(HostError::SerializeUnsupported) => STATES_UNAVAILABLE,
        Some(HostError::StatesNeedHwContext) => STATES_NEED_HW_CONTEXT,
        _ => fallback,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_states_for_this_game_gets_its_own_status() {
        let none = anyhow::Error::from(HostError::SerializeUnsupported).context("slot 1");
        assert_eq!(failure_status(&none, "Save failed"), STATES_UNAVAILABLE);
        let other = anyhow::Error::from(HostError::SerializeFailed);
        assert_eq!(failure_status(&other, "Load failed"), "Load failed");
        let no_context = anyhow::Error::from(HostError::StatesNeedHwContext).context("slot 2");
        assert_eq!(
            failure_status(&no_context, "Save failed"),
            STATES_NEED_HW_CONTEXT
        );
        let hardcore = anyhow::Error::from(HardcoreLoadRefused);
        assert_eq!(failure_status(&hardcore, "Load failed"), STATE_LOAD_REFUSED);
    }
}
