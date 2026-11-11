//! The WebRTC socket (`native` only). matchbox's `MessageLoopFuture` (signaling +
//! ICE + data channels, webrtc on its own smol runtime) runs on a dedicated std
//! thread under `futures::executor::block_on`; the `WebRtcSocket` and both channels
//! stay on the core thread (futures mpsc handles: no lock contended per frame).
//! Channel 0 = ggrs (unreliable, unordered, no retransmits), channel 1 = lobby
//! (reliable). The ICE list is one matchbox entry (one username / credential for all
//! urls); "prefer relay" only puts TURN urls first — matchbox 0.15 has no
//! `ice_transport_policy` knob (spike 00), so it hides nothing. Never logs the url,
//! token, peer ids or ICE data.

use matchbox_socket::{
    ChannelConfig, PeerId, PeerState, RtcIceServerConfig, WebRtcChannel, WebRtcSocket,
    WebRtcSocketBuilder,
};

use crate::netplay_session_env::NetplaySessionEnv;
use crate::peer_channel::{share_channel, PeerChannel, SharedPeerChannel};
use crate::{NetplayError, PeerKey};

const GGRS_CHANNEL: usize = 0;
const LOBBY_CHANNEL: usize = 1;

/// A peer came or went (from matchbox `update_peers`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerChange {
    Joined(PeerKey),
    Left(PeerKey),
}

fn key(peer: PeerId) -> PeerKey {
    PeerKey(peer.0.as_u128())
}

fn peer_id(key: PeerKey) -> PeerId {
    PeerId(uuid::Uuid::from_u128(key.0))
}

/// matchbox channel as a [`PeerChannel`].
struct MatchboxChannel(WebRtcChannel);

impl PeerChannel for MatchboxChannel {
    fn send(&mut self, peer: PeerKey, packet: Box<[u8]>) {
        // Err = message loop gone; the next peer poll reports `Closed`.
        let _ = self.0.try_send(packet.into(), peer_id(peer));
    }

    fn receive(&mut self) -> Vec<(PeerKey, Box<[u8]>)> {
        let packets = self.0.receive();
        let boxed = |packet: Vec<u8>| packet.into_boxed_slice();
        packets
            .into_iter()
            .map(|(peer, packet)| (key(peer), boxed(packet.into())))
            .collect()
    }
}

/// The room's WebRTC socket.
pub struct NetplaySocket {
    socket: WebRtcSocket,
    ggrs: SharedPeerChannel,
    lobby: SharedPeerChannel,
}

/// All ICE urls in one entry; TURN first when `prefer_relay`.
fn ice_config(env: &NetplaySessionEnv, prefer_relay: bool) -> RtcIceServerConfig {
    let mut servers: Vec<_> = env.ice_servers.iter().collect();
    if prefer_relay {
        servers.sort_by_key(|server| !server.is_turn());
    }
    let turn = env.ice_servers.iter().find(|s| s.username.is_some());
    RtcIceServerConfig {
        urls: servers
            .iter()
            .flat_map(|s| s.urls.iter().cloned())
            .collect(),
        username: turn.and_then(|s| s.username.clone()),
        credential: turn.and_then(|s| s.credential.clone()),
    }
}

impl NetplaySocket {
    /// Connects to the room in `env`; returns at once (peers arrive via `poll_peers`).
    pub fn open(env: &NetplaySessionEnv, prefer_relay: bool) -> Result<Self, NetplayError> {
        env.validate()?;
        let mut builder = WebRtcSocketBuilder::new(env.ws_url())
            .add_channel(ChannelConfig::unreliable())
            .add_channel(ChannelConfig::reliable())
            .reconnect_attempts(Some(3));
        if !env.ice_servers.is_empty() {
            builder = builder.ice_server(ice_config(env, prefer_relay));
        }
        let (mut socket, message_loop) = builder.build();
        std::thread::Builder::new()
            .name("netplay-socket".into())
            .spawn(move || {
                if futures::executor::block_on(message_loop).is_err() {
                    // The error text may contain the signaling url (token): not logged.
                    tracing::warn!("netplay socket loop ended with an error");
                }
            })
            .map_err(|_| NetplayError::Socket("cannot start the network thread".into()))?;
        let mut take = |index| {
            let channel = socket
                .take_channel(index)
                .map_err(|_| NetplayError::Socket("data channel missing".into()))?;
            Ok::<_, NetplayError>(share_channel(MatchboxChannel(channel)))
        };
        let (ggrs, lobby) = (take(GGRS_CHANNEL)?, take(LOBBY_CHANNEL)?);
        Ok(Self {
            socket,
            ggrs,
            lobby,
        })
    }

    /// Own id once the signaling server assigned it.
    pub fn local_peer(&mut self) -> Option<PeerKey> {
        self.socket.id().map(key)
    }

    /// Peer changes since the last call; `Err` once the socket closed for good.
    pub fn poll_peers(&mut self) -> Result<Vec<PeerChange>, NetplayError> {
        let changes = self
            .socket
            .try_update_peers()
            .map_err(|_| NetplayError::Socket("connection to the room closed".into()))?;
        Ok(changes
            .into_iter()
            .map(|(peer, state)| match state {
                PeerState::Connected => PeerChange::Joined(key(peer)),
                PeerState::Disconnected => PeerChange::Left(key(peer)),
            })
            .collect())
    }

    /// The ggrs channel handle (cloned into every `RollbackDriver` session).
    pub fn ggrs_channel(&self) -> SharedPeerChannel {
        self.ggrs.clone()
    }

    /// The reliable lobby channel (for `LobbyLink`).
    pub fn lobby_channel(&self) -> SharedPeerChannel {
        self.lobby.clone()
    }

    /// TURN relayed link to `peer`; `None` when unknown / gone.
    pub fn is_relayed(&self, peer: PeerKey) -> Option<bool> {
        self.socket.is_relayed(peer_id(peer))
    }
}

impl Drop for NetplaySocket {
    /// Closes the signaling connection so the message loop (and its thread) ends.
    fn drop(&mut self) {
        self.socket.close();
    }
}

impl std::fmt::Debug for NetplaySocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetplaySocket").finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "socket_thread_tests.rs"]
mod tests;
