//! In-memory network for tests (no sockets): every endpoint is a [`PeerChannel`];
//! packets wait `latency` ticks (plus 0–2 random ticks with `reorder`) and are lost
//! with `loss_pct` % probability (deterministic xorshift). A lossless, in-order
//! instance doubles as the reliable lobby channel.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::peer_channel::{share_channel, PeerChannel, SharedPeerChannel};
use crate::PeerKey;

struct Packet {
    deliver_at: u64,
    from: PeerKey,
    bytes: Box<[u8]>,
}

struct NetInner {
    clock: u64,
    inboxes: HashMap<PeerKey, Vec<Packet>>,
    loss_pct: u64,
    latency: u64,
    reorder: bool,
    rng: u64,
}

impl NetInner {
    fn random(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
}

/// The shared medium.
#[derive(Clone)]
pub(crate) struct FakeNet(Rc<RefCell<NetInner>>);

impl FakeNet {
    pub fn new(loss_pct: u64, latency: u64, reorder: bool) -> Self {
        Self(Rc::new(RefCell::new(NetInner {
            clock: 0,
            inboxes: HashMap::new(),
            loss_pct,
            latency,
            reorder,
            rng: 0x9E37_79B9_7F4A_7C15,
        })))
    }

    /// Lossless, in-order, no delay (the reliable lobby channel).
    pub fn reliable() -> Self {
        Self::new(0, 0, false)
    }

    /// Endpoint of `me`.
    pub fn endpoint(&self, me: PeerKey) -> SharedPeerChannel {
        share_channel(FakeChannel {
            net: self.clone(),
            me,
        })
    }

    /// One tick of simulated time passes.
    pub fn tick(&self) {
        self.0.borrow_mut().clock += 1;
    }
}

struct FakeChannel {
    net: FakeNet,
    me: PeerKey,
}

impl PeerChannel for FakeChannel {
    fn send(&mut self, peer: PeerKey, bytes: Box<[u8]>) {
        let mut net = self.net.0.borrow_mut();
        if net.random() % 100 < net.loss_pct {
            return;
        }
        let jitter = if net.reorder { net.random() % 3 } else { 0 };
        let deliver_at = net.clock + net.latency + jitter;
        let from = self.me;
        net.inboxes.entry(peer).or_default().push(Packet {
            deliver_at,
            from,
            bytes,
        });
    }

    fn receive(&mut self) -> Vec<(PeerKey, Box<[u8]>)> {
        let mut net = self.net.0.borrow_mut();
        let now = net.clock;
        let inbox = net.inboxes.entry(self.me).or_default();
        let (due, later): (Vec<Packet>, Vec<Packet>) = std::mem::take(inbox)
            .into_iter()
            .partition(|p| p.deliver_at <= now);
        *inbox = later;
        due.into_iter().map(|p| (p.from, p.bytes)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivers_after_latency_and_loses_some() {
        let net = FakeNet::new(0, 2, false);
        let (a, b) = (net.endpoint(PeerKey(1)), net.endpoint(PeerKey(2)));
        a.borrow_mut().send(PeerKey(2), Box::new([7]));
        net.tick();
        assert!(b.borrow_mut().receive().is_empty());
        net.tick();
        assert_eq!(
            b.borrow_mut().receive(),
            vec![(PeerKey(1), Box::from([7u8]))]
        );
        let lossy = FakeNet::new(50, 0, true);
        let (c, d) = (lossy.endpoint(PeerKey(1)), lossy.endpoint(PeerKey(2)));
        for i in 0..200u8 {
            c.borrow_mut().send(PeerKey(2), Box::new([i]));
        }
        (0..3).for_each(|_| lossy.tick());
        let got = d.borrow_mut().receive().len();
        assert!((50..150).contains(&got), "{got}");
    }

    #[test]
    fn lobby_link_over_the_reliable_net() {
        use crate::lobby_link::LobbyLink;
        use crate::lobby_protocol::LobbyMessage;
        let net = FakeNet::reliable();
        let (mut a, mut b) = (
            LobbyLink::new(net.endpoint(PeerKey(1))),
            LobbyLink::new(net.endpoint(PeerKey(2))),
        );
        for nonce in 0..5 {
            a.send(&[PeerKey(2)], &LobbyMessage::Ping { nonce }).ok();
        }
        let got: Vec<_> = b
            .receive()
            .into_iter()
            .map(|(peer, msg)| (peer, msg.ok()))
            .collect();
        let want: Vec<_> = (0..5)
            .map(|nonce| (PeerKey(1), Some(LobbyMessage::Ping { nonce })))
            .collect();
        assert_eq!(got, want, "in order, none lost");
    }
}
