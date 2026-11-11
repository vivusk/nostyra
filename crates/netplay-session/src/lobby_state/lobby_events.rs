//! Inputs, outputs and phases of the lobby state machine.

use crate::lobby_protocol::{LobbyMessage, RefusalReason};
use crate::peer_handles::PeerHandles;
use crate::room_api_types::SeatRole;
use crate::session_fingerprint::SessionFingerprint;
use crate::sync_transfer::SyncPayload;
use crate::{NetplayError, PeerKey};

/// Where the lobby is. `Resyncing` = drivers dropped, waiting for every
/// `ResyncReady` (host) or for the host's `SyncBegin` (peers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyPhase {
    /// Socket not connected yet (no own `PeerKey`).
    Connecting,
    /// Guest: `Hello` sent, waiting for the host's `Welcome`.
    Handshaking,
    InLobby,
    Syncing,
    /// Guest: sync loaded and acked, waiting for `Start`.
    Starting,
    Running,
    Resyncing,
    Ended(EndReason),
}

/// Why the session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndReason {
    HostLeft,
    /// The host refused us (or we refused the host's sync data).
    Refused(RefusalReason),
    /// We quit.
    Left,
    Error(String),
}

/// Static facts of this peer's session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LobbyConfig {
    pub role: SeatRole,
    pub nickname: String,
    pub fingerprint: SessionFingerprint,
    /// Own SRAM / RTC sizes (the host's transfer must match, F18).
    pub sram_len: u32,
    pub rtc_len: Option<u32>,
    /// `[netplay] input_delay = N` override; `None` = auto from lobby RTT.
    pub input_delay: Option<usize>,
}

/// Something that happened, fed to `LobbyState::apply`.
#[derive(Debug, Clone, PartialEq)]
pub enum LobbyEvent {
    /// The socket got its own id from the signaling server.
    Connected {
        me: PeerKey,
    },
    PeerJoined(PeerKey),
    PeerLeft(PeerKey),
    Message(PeerKey, LobbyMessage),
    /// A packet from this peer could not be decoded (another lobby protocol
    /// version = another Nostyra version, or a malformed message).
    BadMessage(PeerKey, NetplayError),
    /// Host: the local `MUST_INITIALIZE` warm-up finished (or was not needed).
    WarmupDone,
    /// Host pressed Start in the waiting room.
    StartPressed,
    /// Host: answer to [`LobbyEffect::PrepareSync`].
    SyncData(SyncPayload),
    /// Guest: [`LobbyEffect::LoadSync`] applied to the core.
    SyncLoaded,
    /// Host pressed Resync after a desync warning.
    ResyncRequested,
    /// Local quit.
    Quit,
    /// Timers (lobby pings, resync timeout); once per redraw is enough.
    Tick,
}

impl LobbyEvent {
    /// One [`crate::lobby_link::LobbyLink::receive`] item as an event.
    pub fn received(peer: PeerKey, msg: Result<LobbyMessage, NetplayError>) -> Self {
        match msg {
            Ok(msg) => Self::Message(peer, msg),
            Err(err) => Self::BadMessage(peer, err),
        }
    }
}

/// What the runner must do, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum LobbyEffect {
    Send {
        to: Vec<PeerKey>,
        msg: LobbyMessage,
    },
    /// Host: serialize SRAM + RTC + a `Normal` state, answer `SyncData`.
    PrepareSync,
    /// Guest: run `n` warm-up frames with default input before loading the sync.
    RunWarmup(u32),
    /// Guest: load these into the core, then answer `SyncLoaded`.
    LoadSync(SyncPayload),
    StartDriver(StartPlan),
    /// Resync: build the new ggrs session on the same channel (F23).
    RebuildDriver(StartPlan),
    DropDriver,
    /// Roster / phase / progress changed.
    NotifyUi,
}

/// Everything the rollback driver needs from the lobby.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartPlan {
    pub handles: PeerHandles,
    pub me: PeerKey,
    pub role: SeatRole,
    pub input_delay: usize,
    /// ggrs packet epoch (one per session, F23).
    pub epoch: u8,
}

impl StartPlan {
    /// Own ggrs player handle; `None` for a spectator.
    pub fn local_handle(&self) -> Option<usize> {
        let players = self.handles.players();
        players.iter().find(|p| p.0 == self.me).map(|p| p.1)
    }
}
