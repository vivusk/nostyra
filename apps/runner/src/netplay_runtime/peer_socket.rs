//! The room connection as the runtime sees it: own id, peers coming and going, the
//! ggrs and lobby channels, relay state. `NetplaySocket` (matchbox / WebRTC) online,
//! an in-memory mesh in the in-process tests.

use netplay_session::peer_channel::SharedPeerChannel;
use netplay_session::socket_thread::{NetplaySocket, PeerChange};
use netplay_session::{NetplayError, PeerKey};

pub(crate) trait PeerSocket {
    /// Own id once the signaling server assigned it.
    fn local_peer(&mut self) -> Option<PeerKey>;
    /// Peer changes since the last call; `Err` once the room connection closed.
    fn poll_peers(&mut self) -> Result<Vec<PeerChange>, NetplayError>;
    fn ggrs_channel(&self) -> SharedPeerChannel;
    fn lobby_channel(&self) -> SharedPeerChannel;
    /// TURN relayed link to `peer`, when known.
    fn is_relayed(&self, peer: PeerKey) -> Option<bool>;
}

impl PeerSocket for NetplaySocket {
    fn local_peer(&mut self) -> Option<PeerKey> {
        NetplaySocket::local_peer(self)
    }

    fn poll_peers(&mut self) -> Result<Vec<PeerChange>, NetplayError> {
        NetplaySocket::poll_peers(self)
    }

    fn ggrs_channel(&self) -> SharedPeerChannel {
        NetplaySocket::ggrs_channel(self)
    }

    fn lobby_channel(&self) -> SharedPeerChannel {
        NetplaySocket::lobby_channel(self)
    }

    fn is_relayed(&self, peer: PeerKey) -> Option<bool> {
        NetplaySocket::is_relayed(self, peer)
    }
}
