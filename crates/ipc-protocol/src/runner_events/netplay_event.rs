//! Netplay events on the runner's stdout (`RunnerEvent::Netplay`, M20): lobby
//! roster, sync progress, start, periodic stats, desync, a peer leaving and the end
//! of the session. Never carries a room code, token, address or ICE data: the
//! launcher knows the code from its own REST call.

use serde::{Deserialize, Serialize};

/// The runner's seat in the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetplayRole {
    Host,
    Player,
    Spectator,
}

/// One roster line of the waiting room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetplayRosterEntry {
    pub nick: String,
    pub role: NetplayRole,
    /// Lobby round trip to the host (the host's own line: `None`).
    pub ping_ms: Option<u32>,
    /// TURN relayed link, when known.
    pub relayed: Option<bool>,
}

/// Why a netplay session ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum NetplayEndReason {
    HostLeft,
    /// The core / game cannot run netplay (registry or the loaded core's quirks).
    Ineligible {
        reason: String,
    },
    /// The host refused us (fingerprint, room full, started …); one line per cause.
    Refused {
        mismatches: Vec<String>,
    },
    Quit,
    Error {
        message: String,
    },
}

/// Runner → launcher netplay notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum NetplayEvent {
    /// Waiting room changed (roster, phase).
    Lobby {
        role: NetplayRole,
        roster: Vec<NetplayRosterEntry>,
        /// The core is offered as Experimental (badge + warning).
        experimental: bool,
    },
    /// Initial / resync state transfer, 0 ..= 100 %.
    Syncing {
        percent: u8,
    },
    /// ggrs frame 0 (the host's launcher then calls `/start`).
    Started {
        players: u8,
        input_delay: u8,
    },
    /// Every 2 s while running.
    Stats {
        ping_ms: Option<u32>,
        rollback_fps: u32,
        relayed: bool,
    },
    Desync {
        frame: i32,
    },
    PeerLeft {
        nick: String,
    },
    Ended {
        reason: NetplayEndReason,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner_events::{decode_event_line, encode_event_line, RunnerEvent};

    #[test]
    fn netplay_events_round_trip_with_stable_tags() {
        let events = [
            NetplayEvent::Lobby {
                role: NetplayRole::Host,
                roster: vec![NetplayRosterEntry {
                    nick: "Ann".into(),
                    role: NetplayRole::Player,
                    ping_ms: Some(40),
                    relayed: Some(false),
                }],
                experimental: true,
            },
            NetplayEvent::Syncing { percent: 50 },
            NetplayEvent::Started {
                players: 2,
                input_delay: 2,
            },
            NetplayEvent::Stats {
                ping_ms: None,
                rollback_fps: 3,
                relayed: true,
            },
            NetplayEvent::Desync { frame: 240 },
            NetplayEvent::PeerLeft { nick: "Bob".into() },
            NetplayEvent::Ended {
                reason: NetplayEndReason::Refused {
                    mismatches: vec!["Different core version: 1.62 vs 1.63".into()],
                },
            },
            NetplayEvent::Ended {
                reason: NetplayEndReason::HostLeft,
            },
        ];
        for event in events {
            let line = encode_event_line(&RunnerEvent::Netplay(event.clone()));
            assert_eq!(
                decode_event_line(line.trim()),
                Some(RunnerEvent::Netplay(event))
            );
        }
        let started = RunnerEvent::Netplay(NetplayEvent::Started {
            players: 3,
            input_delay: 1,
        });
        assert_eq!(
            encode_event_line(&started).trim(),
            r#"{"type":"Netplay","event":"Started","players":3,"input_delay":1}"#
        );
        let ended = r#"{"type":"Netplay","event":"Ended","reason":{"kind":"Quit"}}"#;
        assert_eq!(
            decode_event_line(ended),
            Some(RunnerEvent::Netplay(NetplayEvent::Ended {
                reason: NetplayEndReason::Quit
            }))
        );
    }
}
