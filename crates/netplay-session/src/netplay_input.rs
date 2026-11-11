//! The per-frame wire input (`netplay-wire`, F25: no `input-mapping` dependency) and
//! the ggrs prediction rule used for remote players whose input has not arrived.

pub use netplay_wire::{NetplayInput, NETPLAY_PORTS, WIRE_VERSION};

/// ggrs prediction: repeat the remote player's last known input (held buttons stay
/// held, the common case in games).
#[cfg(feature = "native")]
pub type InputPredictor = ggrs::PredictRepeatLast;
