//! In-memory room for the in-process netplay tests: every peer sees every other
//! (full mesh like the signaling server), packets on both channels arrive after a
//! fixed number of mesh ticks (latency), a peer can leave.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use netplay_session::peer_channel::{share_channel, PeerChannel, SharedPeerChannel};
use netplay_session::socket_thread::PeerChange;
use netplay_session::{NetplayError, PeerKey};

use super::peer_socket::PeerSocket;

struct Packet {
    deliver_at: u64,
    channel: usize,
    from: PeerKey,
    to: PeerKey,
    bytes: Box<[u8]>,
}

#[derive(Default)]
struct MeshState {
    tick: u64,
    latency: u64,
    alive: BTreeSet<PeerKey>,
    queue: Vec<Packet>,
}

/// The shared room.
#[derive(Clone, Default)]
pub(crate) struct LocalMesh(Rc<RefCell<MeshState>>);

impl LocalMesh {
    pub(crate) fn new(latency_ticks: u64) -> Self {
        let mesh = Self::default();
        mesh.0.borrow_mut().latency = latency_ticks;
        mesh
    }

    /// Adds a peer and returns its socket.
    pub(crate) fn join(&self, id: u128) -> Box<dyn PeerSocket> {
        let me = PeerKey(id);
        self.0.borrow_mut().alive.insert(me);
        let channel = |index| share_channel(MeshChannel(self.clone(), me, index));
        Box::new(MeshSocket {
            mesh: self.clone(),
            me,
            known: BTreeSet::new(),
            ggrs: channel(0),
            lobby: channel(1),
        })
    }

    /// One time step: packets due by now become receivable.
    pub(crate) fn advance(&self) {
        self.0.borrow_mut().tick += 1;
    }

    pub(crate) fn leave(&self, id: u128) {
        self.0.borrow_mut().alive.remove(&PeerKey(id));
    }
}

struct MeshChannel(LocalMesh, PeerKey, usize);

impl PeerChannel for MeshChannel {
    fn send(&mut self, peer: PeerKey, packet: Box<[u8]>) {
        let mut state = self.0 .0.borrow_mut();
        if !state.alive.contains(&self.1) {
            return;
        }
        let deliver_at = state.tick + state.latency;
        state.queue.push(Packet {
            deliver_at,
            channel: self.2,
            from: self.1,
            to: peer,
            bytes: packet,
        });
    }

    fn receive(&mut self) -> Vec<(PeerKey, Box<[u8]>)> {
        let mut state = self.0 .0.borrow_mut();
        let (tick, me, channel) = (state.tick, self.1, self.2);
        let mut out = Vec::new();
        let mut kept = Vec::with_capacity(state.queue.len());
        for packet in state.queue.drain(..) {
            if packet.to == me && packet.channel == channel && packet.deliver_at <= tick {
                out.push((packet.from, packet.bytes));
            } else {
                kept.push(packet);
            }
        }
        state.queue = kept;
        out
    }
}

struct MeshSocket {
    mesh: LocalMesh,
    me: PeerKey,
    known: BTreeSet<PeerKey>,
    ggrs: SharedPeerChannel,
    lobby: SharedPeerChannel,
}

impl PeerSocket for MeshSocket {
    fn local_peer(&mut self) -> Option<PeerKey> {
        Some(self.me)
    }

    fn poll_peers(&mut self) -> Result<Vec<PeerChange>, NetplayError> {
        // No clone: the hot-path allocation test polls this every redraw.
        let state = self.mesh.0.borrow();
        let mut changes = Vec::new();
        for &peer in state.alive.iter().filter(|&&p| p != self.me) {
            if self.known.insert(peer) {
                changes.push(PeerChange::Joined(peer));
            }
        }
        self.known.retain(|peer| {
            let alive = state.alive.contains(peer);
            if !alive {
                changes.push(PeerChange::Left(*peer));
            }
            alive
        });
        Ok(changes)
    }

    fn ggrs_channel(&self) -> SharedPeerChannel {
        self.ggrs.clone()
    }

    fn lobby_channel(&self) -> SharedPeerChannel {
        self.lobby.clone()
    }

    fn is_relayed(&self, _peer: PeerKey) -> Option<bool> {
        Some(false)
    }
}
