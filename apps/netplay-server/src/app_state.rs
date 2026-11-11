//! State shared by every handler.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use crate::room_registry::{RoomLimits, RoomRegistry};
use crate::server_config::ServerConfig;
use crate::signaling_mesh::SignalingMesh;

/// Cloned into each request (all fields are cheap handles).
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub registry: Arc<RoomRegistry>,
    pub mesh: SignalingMesh,
    /// Turns `true` at shutdown: open WebSockets send Close and end.
    pub shutdown: watch::Receiver<bool>,
}

impl AppState {
    pub fn new(config: ServerConfig, shutdown: watch::Receiver<bool>) -> Self {
        let limits = RoomLimits {
            max_rooms: config.max_rooms,
            max_spectators: config.max_spectators,
        };
        Self {
            config: Arc::new(config),
            registry: Arc::new(RoomRegistry::new(limits)),
            mesh: SignalingMesh::default(),
            shutdown,
        }
    }
}

/// Wall clock in Unix seconds (token / TURN expiry).
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
