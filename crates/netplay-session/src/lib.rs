//! Netplay session (M20): everything that is neither "drive a libretro core" nor
//! "draw UI" — room REST client (`rest`), the runner's session env, compatibility
//! fingerprint, lobby protocol + state machine, SRAM / RTC / state transfer, the
//! ggrs rollback driver and the matchbox WebRTC socket (`native`).
//!
//! **Thread model.** The network thread (`socket_thread`) owns matchbox's message
//! loop (signaling, ICE, data channels). Everything else runs on the core thread:
//! `poll_peers()` + `LobbyLink::receive()` + `LobbyState::apply()` once per redraw
//! (the "lobby poll"), and `RollbackDriver::tick()` once per emulated frame. The
//! data channels are lock-free mpsc handles; nothing here blocks the frame loop.
//!
//! **Usage (phase 04 runner, sketch):**
//! ```ignore
//! let env = NetplaySessionEnv::from_env()?;
//! let mut socket = NetplaySocket::open(&env, prefer_relay)?;
//! let mut link = LobbyLink::new(socket.lobby_channel());
//! let mut lobby = LobbyState::new(LobbyConfig { role: env.role, fingerprint, .. });
//! loop { // per redraw
//!     let mut events = vec![LobbyEvent::Tick];
//!     if let Some(me) = socket.local_peer() { events.push(LobbyEvent::Connected { me }) }
//!     events.extend(socket.poll_peers()?.into_iter().map(peer_change_event));
//!     events.extend(link.receive().into_iter().map(|(p, m)| LobbyEvent::received(p, m)));
//!     for effect in events.into_iter().flat_map(|e| lobby.apply(e, Instant::now())) {
//!         match effect {
//!             LobbyEffect::Send { to, msg } => link.send(&to, &msg)?,
//!             LobbyEffect::StartDriver(plan) => driver = Some(RollbackDriver::new(
//!                 &RollbackConfig::new(plan, fps_milli), socket.ggrs_channel())?),
//!             LobbyEffect::RebuildDriver(plan) => driver.rebuild(&RollbackConfig::new(plan, fps_milli), socket.ggrs_channel())?,
//!             _ => { /* PrepareSync → apply(SyncData), LoadSync → apply(SyncLoaded), … */ }
//!         }
//!     }
//!     for i in 0..due { driver.tick(&[local_input], &mut host, i + 1 == due)?; }
//! }
//! ```

#![deny(unsafe_code)]

pub mod desync_checksum;
pub mod lobby_link;
pub mod lobby_protocol;
pub mod lobby_state;
pub mod net_stats;
pub mod netplay_input;
pub mod netplay_session_env;
pub mod peer_channel;
pub mod peer_handles;
mod peer_key;
pub mod rollback_config;
pub mod room_api_rules;
pub mod room_api_types;
pub mod session_fingerprint;
pub mod sync_transfer;

#[cfg(feature = "rest")]
pub mod room_api_client;

#[cfg(feature = "native")]
pub mod rollback_driver;
#[cfg(feature = "native")]
pub mod shared_ggrs_channel;
#[cfg(feature = "native")]
pub mod socket_thread;

#[cfg(all(test, feature = "native"))]
mod fake_socket;

pub use peer_key::PeerKey;

/// Every netplay failure. Texts never contain urls, room codes, tokens or addresses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetplayError {
    #[error("netplay session settings are missing or invalid: {0}")]
    SessionEnv(&'static str),
    #[error("network is switched off (offline mode)")]
    Offline,
    #[error("could not reach the netplay server")]
    Network,
    #[error("the netplay server address is not a ws(s):// or http(s):// url")]
    BadServerUrl,
    #[error("room codes have 6 letters or digits")]
    BadRoomCode,
    #[error("room not found")]
    RoomNotFound,
    #[error("room expired")]
    RoomExpired,
    #[error("room is full")]
    RoomFull,
    #[error("game already started")]
    RoomStarted,
    #[error("this room does not allow spectators")]
    SpectatorsOff,
    #[error("this seat is already connected")]
    AlreadyConnected,
    #[error("the server refused the request (not the host or token expired)")]
    Forbidden,
    #[error("too many requests, try again later")]
    RateLimited { retry_after_secs: Option<u64> },
    #[error("the netplay server is full, try again later")]
    ServerFull,
    #[error("the server refused the request ({0})")]
    BadRequest(String),
    #[error("netplay server error (http {0})")]
    Server(u16),
    #[error("unexpected reply from the netplay server")]
    BadReply,
    #[error("peer runs lobby protocol {found}, this build speaks {expected}: update Nostyra")]
    ProtocolVersion { found: u16, expected: u16 },
    #[error("bad lobby message: {0}")]
    Protocol(String),
    #[error("state transfer failed: {0}")]
    Sync(String),
    #[error("rollback session: {0}")]
    Rollback(String),
    #[error("emulator: {0}")]
    Host(String),
    #[error("network socket: {0}")]
    Socket(String),
}
