//! The packet seam between the session code and a transport: one data channel to
//! every peer (matchbox `WebRtcChannel` online, `fake_socket` in tests). The ggrs
//! channel (unreliable, channel 0) and the lobby channel (reliable, channel 1) are
//! both [`PeerChannel`]s held behind a [`SharedPeerChannel`], so the ggrs channel
//! outlives each ggrs session (F23: `start_p2p_session` takes its socket by value).

use std::cell::RefCell;
use std::rc::Rc;

use crate::PeerKey;

/// Send / receive discrete packets to / from peers. Non-blocking, never panics: a
/// closed transport drops packets (the peer-state poll reports the disconnect).
pub trait PeerChannel {
    /// Queues `packet` for `peer`.
    fn send(&mut self, peer: PeerKey, packet: Box<[u8]>);

    /// Every packet received since the last call, oldest first.
    fn receive(&mut self) -> Vec<(PeerKey, Box<[u8]>)>;
}

/// A channel shared by its owner (`NetplaySocket`) and the sessions built on it.
/// Core-thread only (`Rc`): the network thread never touches it.
pub type SharedPeerChannel = Rc<RefCell<dyn PeerChannel>>;

/// Wraps `channel` for sharing.
pub fn share_channel(channel: impl PeerChannel + 'static) -> SharedPeerChannel {
    Rc::new(RefCell::new(channel))
}
