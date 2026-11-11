//! Per-socket limits for the signaling mesh: a bounded outbound queue (a peer that stops
//! reading is kicked instead of buffering without end) and an inbound message budget
//! (a flooding peer is closed with a policy-violation frame).

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::SinkExt;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;

/// Outbound messages queued per peer (each at most 64 KiB) before it counts as stalled.
pub const QUEUE_CAPACITY: usize = 64;
/// Inbound budget per socket: sustained messages per second, and the burst on top.
const MESSAGES_PER_SECOND: f64 = 50.0;
const MESSAGE_BURST: f64 = 200.0;
/// How long a closing socket may take to flush its queue before it is dropped.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

/// The mesh's handle on one peer's outbound queue.
#[derive(Clone)]
pub struct PeerSender {
    tx: mpsc::Sender<Message>,
    kick: Arc<Notify>,
}

impl PeerSender {
    pub fn new(capacity: usize) -> (Self, mpsc::Receiver<Message>) {
        let (tx, rx) = mpsc::channel(capacity);
        let kick = Arc::new(Notify::new());
        (Self { tx, kick }, rx)
    }

    /// Queues `message`; `false` if it was dropped. A full queue kicks the peer.
    pub fn send(&self, message: Message) -> bool {
        match self.tx.try_send(message) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.kick.notify_one();
                false
            }
            Err(TrySendError::Closed(_)) => false,
        }
    }

    /// Resolves once the queue overflowed (the permit is kept until awaited).
    pub async fn kicked(&self) {
        self.kick.notified().await;
    }
}

/// Writes queued messages to the socket until every [`PeerSender`] is dropped.
pub fn spawn_writer(
    mut sink: SplitSink<WebSocket, Message>,
    mut queue: mpsc::Receiver<Message>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(message) = queue.recv().await {
            if sink.send(message).await.is_err() {
                break;
            }
        }
    })
}

/// Lets the writer flush (e.g. a close frame), then drops it if the socket is stalled.
pub async fn finish_writer(mut writer: JoinHandle<()>) {
    if tokio::time::timeout(FLUSH_TIMEOUT, &mut writer)
        .await
        .is_err()
    {
        writer.abort();
    }
}

/// Token bucket over inbound frames of one socket.
pub struct MessageBudget {
    tokens: f64,
    last: Instant,
}

impl MessageBudget {
    pub fn new(now: Instant) -> Self {
        Self {
            tokens: MESSAGE_BURST,
            last: now,
        }
    }

    /// Spends one message; `false` when the peer is over budget.
    pub fn take(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * MESSAGES_PER_SECOND).min(MESSAGE_BURST);
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn full_queue_drops_and_kicks() {
        let (sender, _queue) = PeerSender::new(2);
        assert!(sender.send(Message::Text("a".into())));
        assert!(sender.send(Message::Text("b".into())));
        assert!(
            !sender.send(Message::Text("c".into())),
            "queue stays bounded"
        );
        tokio::time::timeout(Duration::from_secs(1), sender.kicked())
            .await
            .unwrap_or_else(|_| panic!("stalled peer must be kicked"));
    }

    #[test]
    fn budget_allows_burst_then_refills() {
        let start = Instant::now();
        let mut budget = MessageBudget::new(start);
        let burst = MESSAGE_BURST as usize;
        assert!((0..burst).all(|_| budget.take(start)));
        assert!(!budget.take(start), "over budget");
        assert!(budget.take(start + Duration::from_millis(100)), "refilled");
    }
}
