//! Netplay facts of a core (M20 spike 00): whether rollback netplay is offered, and
//! how a room of more than two players reaches the core (multitap port devices
//! and / or core options) or how few players the system has.

/// Whether a core is offered for online play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetplaySupport {
    /// Not offered (desyncs under rollback, rollback too slow, or `Serialized` states).
    #[default]
    No,
    /// Passed the spike synctest; offered normally.
    Yes,
    /// Passed with test content only; offered with a badge and a warning.
    Experimental,
}

/// `RETRO_DEVICE_JOYPAD`.
pub const DEVICE_JOYPAD: u32 = 1;
/// `RETRO_DEVICE_SUBCLASS(JOYPAD, 0)`: snes9x Multitap, Mesen pad on ports 2–3.
pub const DEVICE_JOYPAD_MULTITAP: u32 = 257;
/// `RETRO_DEVICE_SUBCLASS(JOYPAD, 5)`: Genesis Plus GX Sega Team Player, 3 buttons.
pub const DEVICE_GPGX_TEAM_PLAYER_3B: u32 = 1537;
/// `RETRO_DEVICE_SUBCLASS(JOYPAD, 7)`: Genesis Plus GX Master System Master Tap.
pub const DEVICE_GPGX_MASTER_TAP: u32 = 2049;

/// Players and multitap setup of a core's netplay room, for some of its systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultitapSpec {
    /// System ids this entry is for (empty: every system of the core).
    pub systems: &'static [&'static str],
    /// `RETRO_DEVICE_*` per port when the room has more than two players, replacing
    /// `CoreSpec.port_devices` (empty: unchanged).
    pub devices: &'static [u32],
    /// Core options answered when the room has more than two players (before
    /// `retro_load_game`, so the options hash covers them).
    pub options: &'static [(&'static str, &'static str)],
    /// Largest room (players, spectators not counted) for these systems.
    pub max_players: u8,
}

/// Players of a room when no [`MultitapSpec`] entry matches: two pad ports.
pub const DEFAULT_NETPLAY_PLAYERS: u8 = 2;

/// One player only (handhelds: one unit, no link cable in v1).
pub const SINGLE_PLAYER: MultitapSpec = MultitapSpec {
    systems: &[],
    devices: &[],
    options: &[],
    max_players: 1,
};
