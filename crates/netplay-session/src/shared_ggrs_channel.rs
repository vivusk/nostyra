//! ggrs socket over the shared unreliable channel (F23). ggrs takes its socket by
//! value and the channel must survive `RollbackDriver::rebuild`, so every session
//! gets a [`SharedGgrsChannel`] clone of the same [`SharedPeerChannel`]. Each packet
//! starts with a one-byte session epoch and packets of another epoch are dropped:
//! ggrs filters stale packets by `magic` only after its endpoint synchronized, and
//! an old session's inputs make a new one panic (spike 00 F23). That is also why
//! ggrs packets never go through matchbox's own `NonBlockingSocket` impl.

use ggrs::{Message, NonBlockingSocket};

use crate::peer_channel::SharedPeerChannel;
use crate::PeerKey;

/// One ggrs session's view of the shared channel.
pub struct SharedGgrsChannel {
    channel: SharedPeerChannel,
    epoch: u8,
}

impl SharedGgrsChannel {
    pub fn new(channel: SharedPeerChannel, epoch: u8) -> Self {
        Self { channel, epoch }
    }
}

impl NonBlockingSocket<PeerKey> for SharedGgrsChannel {
    fn send_to(&mut self, msg: &Message, addr: &PeerKey) {
        // Encoding a ggrs message into a Vec cannot fail; drop it if it ever does.
        let Ok(body) = bincode::serde::encode_to_vec(msg, bincode::config::standard()) else {
            return;
        };
        let mut packet = Vec::with_capacity(body.len() + 1);
        packet.push(self.epoch);
        packet.extend_from_slice(&body);
        self.channel
            .borrow_mut()
            .send(*addr, packet.into_boxed_slice());
    }

    fn receive_all_messages(&mut self) -> Vec<(PeerKey, Message)> {
        let packets = self.channel.borrow_mut().receive();
        let config = bincode::config::standard().with_limit::<{ 64 * 1024 }>();
        packets
            .into_iter()
            .filter_map(|(peer, packet)| {
                let (&epoch, body) = packet.split_first()?;
                if epoch != self.epoch {
                    return None;
                }
                let (msg, _) = bincode::serde::decode_from_slice(body, config).ok()?;
                Some((peer, msg))
            })
            .collect()
    }
}
