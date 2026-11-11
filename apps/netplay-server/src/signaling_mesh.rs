//! Full-mesh WebRTC signaling per room, speaking the matchbox protocol: `IdAssigned` on
//! connect, `NewPeer` / `PeerLeft` to the others, `Signal` relayed within the room only.
//! Same behaviour as `matchbox_signaling::topologies::full_mesh`, but empty rooms are
//! dropped (the stock state keeps one map entry per room forever), outbound queues are
//! bounded and inbound messages budgeted per socket (`signaling_peer_io`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use axum::extract::ws::{close_code, CloseFrame, Message, WebSocket};
use futures::StreamExt;
use matchbox_protocol::{JsonPeerEvent, PeerId, PeerRequest};
use matchbox_signaling::common_logic::parse_request;
use matchbox_signaling::ClientRequestError;
use tokio::sync::watch;

use crate::random_ids::log_hash;
use crate::signaling_peer_io::{
    finish_writer, spawn_writer, MessageBudget, PeerSender, QUEUE_CAPACITY,
};

type MeshRooms = HashMap<String, HashMap<PeerId, PeerSender>>;

/// Senders of every live WebSocket, grouped by room id.
#[derive(Clone, Default)]
pub struct SignalingMesh(Arc<Mutex<MeshRooms>>);

impl SignalingMesh {
    fn lock(&self) -> MutexGuard<'_, MeshRooms> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn add(&self, room: &str, peer: PeerId, sender: PeerSender) {
        let mut rooms = self.lock();
        let peers = rooms.entry(room.to_owned()).or_default();
        let event = text(&JsonPeerEvent::NewPeer(peer));
        for other in peers.values() {
            other.send(event.clone());
        }
        peers.insert(peer, sender);
    }

    fn remove(&self, room: &str, peer: PeerId) {
        let mut rooms = self.lock();
        let Some(peers) = rooms.get_mut(room) else {
            return;
        };
        if peers.remove(&peer).is_none() {
            return;
        }
        let event = text(&JsonPeerEvent::PeerLeft(peer));
        for other in peers.values() {
            other.send(event.clone());
        }
        if peers.is_empty() {
            rooms.remove(room);
        }
    }

    fn relay(&self, room: &str, to: PeerId, message: Message) {
        let rooms = self.lock();
        if let Some(sender) = rooms.get(room).and_then(|peers| peers.get(&to)) {
            sender.send(message);
        }
    }

    /// Peers currently in `room` (tests, diagnostics).
    pub fn peer_count(&self, room: &str) -> usize {
        self.lock().get(room).map_or(0, HashMap::len)
    }

    /// Runs one peer's socket until it closes or the server shuts down.
    pub async fn run_peer(
        &self,
        socket: WebSocket,
        room: String,
        peer: PeerId,
        mut shutdown: watch::Receiver<bool>,
    ) {
        let (sink, mut receiver) = socket.split();
        let (sender, queue) = PeerSender::new(QUEUE_CAPACITY);
        let writer = spawn_writer(sink, queue);
        sender.send(text(&JsonPeerEvent::IdAssigned(peer)));
        self.add(&room, peer, sender.clone());
        let mut budget = MessageBudget::new(Instant::now());
        loop {
            let next = tokio::select! {
                next = receiver.next() => next,
                () = sender.kicked() => {
                    tracing::info!(room = %log_hash(&room), "stalled peer dropped");
                    break;
                }
                () = shutting_down(&mut shutdown) => {
                    sender.send(Message::Close(None));
                    break;
                }
            };
            let Some(request) = next else { break };
            if !budget.take(Instant::now()) {
                tracing::info!(room = %log_hash(&room), "flooding peer closed");
                sender.send(Message::Close(Some(CloseFrame {
                    code: close_code::POLICY,
                    reason: "message rate".into(),
                })));
                break;
            }
            match parse_request(request) {
                Ok(PeerRequest::Signal { receiver: to, data }) => {
                    let event = JsonPeerEvent::Signal { sender: peer, data };
                    self.relay(&room, to, text(&event));
                }
                Ok(PeerRequest::KeepAlive) => {}
                // Pings / binary / bad JSON: ignore, the peer stays connected.
                Err(ClientRequestError::Json(_) | ClientRequestError::UnsupportedType(_)) => {}
                Err(ClientRequestError::Close | ClientRequestError::Axum(_)) => break,
            }
        }
        self.remove(&room, peer);
        drop(sender);
        finish_writer(writer).await;
    }
}

/// Resolves when shutdown starts; never if the server dropped the switch without it.
async fn shutting_down(shutdown: &mut watch::Receiver<bool>) {
    if shutdown.wait_for(|stop| *stop).await.is_err() {
        std::future::pending::<()>().await;
    }
}

fn text(event: &JsonPeerEvent) -> Message {
    Message::Text(event.to_string().into())
}
