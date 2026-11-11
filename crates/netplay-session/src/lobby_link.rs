//! The lobby side of the socket: encodes [`LobbyMessage`]s onto the reliable
//! channel and decodes what arrived. The core thread calls [`LobbyLink::receive`]
//! once per redraw and feeds the messages to `LobbyState::apply`.

use crate::lobby_protocol::{decode_lobby, encode_lobby, LobbyMessage};
use crate::peer_channel::SharedPeerChannel;
use crate::{NetplayError, PeerKey};

/// Reliable lobby channel endpoint.
pub struct LobbyLink {
    channel: SharedPeerChannel,
}

impl LobbyLink {
    pub fn new(channel: SharedPeerChannel) -> Self {
        Self { channel }
    }

    /// Sends `msg` to every peer in `to` (encoded once).
    pub fn send(&mut self, to: &[PeerKey], msg: &LobbyMessage) -> Result<(), NetplayError> {
        let packet = encode_lobby(msg)?;
        let mut channel = self.channel.borrow_mut();
        for &peer in to {
            channel.send(peer, packet.clone());
        }
        Ok(())
    }

    /// Every message received since the last call; undecodable packets come back as
    /// errors (`ProtocolVersion` = the peer runs another Nostyra version); feed each
    /// item to the lobby as `LobbyEvent::received(peer, msg)`.
    pub fn receive(&mut self) -> Vec<(PeerKey, Result<LobbyMessage, NetplayError>)> {
        let packets = self.channel.borrow_mut().receive();
        packets
            .into_iter()
            .map(|(peer, packet)| (peer, decode_lobby(&packet)))
            .collect()
    }
}
