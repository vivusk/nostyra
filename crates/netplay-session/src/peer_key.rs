//! Peer identity inside a room: matchbox's `PeerId` (a uuid) as a plain `u128`, so the
//! lobby protocol, the state machine and the ggrs `Config::Address` stay free of
//! matchbox (`--no-default-features`, tests on an in-memory socket).

use serde::{Deserialize, Serialize};

/// One peer of the room. Ordering = matchbox `PeerId` (uuid) ordering.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PeerKey(pub u128);

impl std::fmt::Debug for PeerKey {
    /// Only the last 4 hex digits: enough to tell peers apart in a log line, not a
    /// full id another client could replay.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Peer(…{:04x})", self.0 & 0xffff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_shows_a_short_suffix_only() {
        let text = format!("{:?}", PeerKey(0x1234_5678_9abc_def0_1122_3344_5566_beef));
        assert_eq!(text, "Peer(…beef)");
    }
}
