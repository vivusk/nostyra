//! Background sweep: dead rooms out of the registry, idle buckets out of the limiters.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::rate_limits::RateLimits;
use crate::room_registry::RoomRegistry;

/// How often the sweep runs (the GC rules are minute-scale, 10 s is precise enough).
pub const GC_PERIOD: Duration = Duration::from_secs(10);

/// Spawns the sweep on the current tokio runtime; it runs until the runtime stops.
pub fn spawn_room_gc(
    registry: Arc<RoomRegistry>,
    limits: RateLimits,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(GC_PERIOD);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let removed = registry.collect_garbage(Instant::now());
            if removed > 0 {
                tracing::info!(removed, rooms = registry.room_count(), "rooms collected");
            }
            limits.retain_recent();
        }
    })
}
