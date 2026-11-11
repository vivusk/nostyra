//! Messages on the reliable lobby channel (matchbox channel 1): handshake, roster,
//! initial / resync state transfer, start, resync, leave, ping. Wire form = a `u16`
//! little-endian protocol version, then the bincode 2 (serde, standard config)
//! body; another version is refused with [`NetplayError::ProtocolVersion`].

use serde::{Deserialize, Serialize};

use crate::room_api_types::SeatRole;
use crate::session_fingerprint::{FingerprintMismatch, SessionFingerprint};
use crate::{NetplayError, PeerKey};

/// Version of the lobby protocol (bump on any message change).
pub const LOBBY_PROTOCOL_VERSION: u16 = 1;
/// Largest accepted message (a sync chunk plus framing fits easily).
pub const MAX_LOBBY_MESSAGE_BYTES: usize = 256 * 1024;
/// Sync chunk payload size (well under the SCTP message limit).
pub const SYNC_CHUNK_BYTES: usize = 16 * 1024;

/// One roster line as the host announces it (join order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyMember {
    pub peer: PeerKey,
    pub nickname: String,
    pub role: SeatRole,
    /// ggrs handle for host / players (host = 0); spectators get theirs at `Start`.
    pub handle: Option<usize>,
    /// Lobby round trip to the host, ms (host's own line: `None`).
    pub rtt_ms: Option<u32>,
}

/// Why the host refused a joiner (shown to the joiner).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusalReason {
    Mismatch(Vec<FingerprintMismatch>),
    /// `Hello` after the host started syncing (F22).
    Started,
    /// Player cap or the 8-spectator cap reached (F28).
    RoomFull,
    SramSize {
        host: u32,
        guest: u32,
    },
    RtcMismatch,
    WarmupFrames {
        host: u32,
        guest: u32,
    },
    /// The host could not decode our messages (another Nostyra version).
    Protocol,
}

impl RefusalReason {
    /// Text for the joiner's UI.
    pub fn text(&self) -> String {
        match self {
            Self::Mismatch(diff) => {
                let lines: Vec<String> = diff.iter().map(FingerprintMismatch::text).collect();
                lines.join("; ")
            }
            Self::Started => "Game already started".to_owned(),
            Self::RoomFull => "Room is full".to_owned(),
            Self::SramSize { host, guest } => {
                format!("Different save memory size: {host} vs {guest} bytes")
            }
            Self::RtcMismatch => "Different clock (RTC) memory".to_owned(),
            Self::WarmupFrames { host, guest } => {
                format!("Different start-up frames: {host} vs {guest}")
            }
            Self::Protocol => "Different Nostyra version: update Nostyra".to_owned(),
        }
    }
}

/// Which blob a sync chunk belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyncKind {
    Sram,
    Rtc,
    State,
}

/// crc32 of each transferred blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCrcs {
    pub sram: u32,
    pub rtc: Option<u32>,
    pub state: u32,
}

/// Every lobby message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobbyMessage {
    /// Guest → each new peer (only the host answers, F31).
    Hello {
        fingerprint: SessionFingerprint,
        nickname: String,
        role: SeatRole,
    },
    Welcome {
        roster: Vec<LobbyMember>,
        your_handle: Option<usize>,
    },
    Refused {
        reason: RefusalReason,
    },
    RosterUpdate {
        roster: Vec<LobbyMember>,
    },
    /// `state_len` > 0 always (the host always sends a state).
    SyncBegin {
        sram_len: u32,
        rtc_len: Option<u32>,
        state_len: u32,
        warmup_frames: u32,
    },
    SyncChunk {
        kind: SyncKind,
        offset: u32,
        bytes: Vec<u8>,
    },
    SyncDone {
        crc32s: SyncCrcs,
    },
    SyncAck,
    /// The actual roster of the ggrs session; `epoch` tags its ggrs packets.
    Start {
        input_delay: usize,
        epoch: u8,
        players: Vec<(PeerKey, usize)>,
        spectators: Vec<(PeerKey, usize)>,
    },
    /// Host → everyone: drop the ggrs session, keep the socket (F23).
    Resync,
    /// Peer → host: driver dropped.
    ResyncReady,
    Leave,
    Ping {
        nonce: u64,
    },
    Pong {
        nonce: u64,
    },
}

fn config() -> impl bincode::config::Config {
    bincode::config::standard().with_limit::<MAX_LOBBY_MESSAGE_BYTES>()
}

/// Encodes `msg` with the version prefix.
pub fn encode_lobby(msg: &LobbyMessage) -> Result<Box<[u8]>, NetplayError> {
    let body = bincode::serde::encode_to_vec(msg, config())
        .map_err(|_| NetplayError::Protocol("cannot encode lobby message".into()))?;
    let mut packet = Vec::with_capacity(body.len() + 2);
    packet.extend_from_slice(&LOBBY_PROTOCOL_VERSION.to_le_bytes());
    packet.extend_from_slice(&body);
    Ok(packet.into_boxed_slice())
}

/// Decodes one packet; refuses other versions, oversized and trailing bytes.
pub fn decode_lobby(packet: &[u8]) -> Result<LobbyMessage, NetplayError> {
    if packet.len() > MAX_LOBBY_MESSAGE_BYTES {
        return Err(NetplayError::Protocol("lobby message too large".into()));
    }
    let (tag, body) = packet
        .split_first_chunk::<2>()
        .ok_or_else(|| NetplayError::Protocol("short lobby message".into()))?;
    let found = u16::from_le_bytes(*tag);
    if found != LOBBY_PROTOCOL_VERSION {
        return Err(NetplayError::ProtocolVersion {
            found,
            expected: LOBBY_PROTOCOL_VERSION,
        });
    }
    let (msg, read) = bincode::serde::decode_from_slice(body, config())
        .map_err(|_| NetplayError::Protocol("malformed lobby message".into()))?;
    if read != body.len() {
        return Err(NetplayError::Protocol("trailing bytes".into()));
    }
    Ok(msg)
}

#[cfg(test)]
#[path = "lobby_protocol_tests.rs"]
mod tests;
