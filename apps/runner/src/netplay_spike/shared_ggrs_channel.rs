//! ggrs socket over a matchbox channel kept behind a shared handle (F23): the
//! channel outlives each `P2PSession` (`take_channel` hands it out once,
//! `start_p2p_session` takes the socket by value). Every packet carries a one-byte
//! session epoch; packets of another epoch are dropped, because ggrs only filters
//! stale packets by `magic` once its endpoint is synchronized.

use std::cell::RefCell;
use std::rc::Rc;

use ggrs::{Message, NonBlockingSocket};
use matchbox_socket::{PeerId, WebRtcChannel};

/// The channel shared by successive sessions.
pub(crate) type SharedChannel = Rc<RefCell<WebRtcChannel>>;

/// One session's view of the shared channel.
pub(crate) struct SharedGgrsChannel {
    channel: SharedChannel,
    epoch: u8,
}

impl SharedGgrsChannel {
    pub(crate) fn new(channel: SharedChannel, epoch: u8) -> Self {
        Self { channel, epoch }
    }
}

impl NonBlockingSocket<PeerId> for SharedGgrsChannel {
    fn send_to(&mut self, msg: &Message, addr: &PeerId) {
        let Ok(body) = bincode::serde::encode_to_vec(msg, bincode::config::standard()) else {
            return;
        };
        let mut packet = Vec::with_capacity(body.len() + 1);
        packet.push(self.epoch);
        packet.extend_from_slice(&body);
        self.channel.borrow_mut().send(packet.into(), *addr);
    }

    fn receive_all_messages(&mut self) -> Vec<(PeerId, Message)> {
        let packets = self.channel.borrow_mut().receive();
        packets
            .into_iter()
            .filter(|(_, packet)| packet.first() == Some(&self.epoch))
            .filter_map(|(peer, packet)| {
                let config = bincode::config::standard();
                let (msg, _) = bincode::serde::decode_from_slice(&packet[1..], config).ok()?;
                Some((peer, msg))
            })
            .collect()
    }
}
