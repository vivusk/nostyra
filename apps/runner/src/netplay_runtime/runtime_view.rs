//! What the overlay, the HUD and the launcher see of the session: roster lines,
//! waiting-room facts, stall / desync / peer-left warnings, the 2-second stats
//! (`Netplay{Stats}`) and lobby change events.

use std::time::{Duration, Instant};

use ipc_protocol::{NetplayEvent, NetplayRole, NetplayRosterEntry};
use netplay_session::lobby_protocol::LobbyMember;
use netplay_session::lobby_state::{EndReason, LobbyPhase};
use netplay_session::room_api_types::SeatRole;

use super::hud_stats::{HudStats, STATS_EVERY};
use super::NetplayRuntime;

/// A peer stalls the game this long before the HUD names it.
const STALL_WARNING: Duration = Duration::from_millis(500);
/// "<nick> left" stays this long.
const PEER_LEFT_SHOWN: Duration = Duration::from_secs(4);

/// One waiting-room roster line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RosterLine {
    pub nick: String,
    pub role: SeatRole,
    pub ping_ms: Option<u32>,
}

fn ipc_role(role: SeatRole) -> NetplayRole {
    match role {
        SeatRole::Host => NetplayRole::Host,
        SeatRole::Player => NetplayRole::Player,
        SeatRole::Spectator => NetplayRole::Spectator,
    }
}

impl NetplayRuntime {
    /// `Netplay{Lobby}` when the roster changed; `Syncing` progress while syncing.
    pub(super) fn emit_lobby_changes(&mut self) {
        // Compared in place: the event (and its strings) is built on a change only.
        let roster = self.lobby.roster();
        let last = &self.stats.last_roster;
        let same = |(m, e): (&LobbyMember, &NetplayRosterEntry)| {
            e.nick == m.nickname
                && e.role == ipc_role(m.role)
                && e.ping_ms == m.rtt_ms
                && e.relayed == self.socket.is_relayed(m.peer) // own line: None
        };
        if roster.len() != last.len() || !roster.iter().zip(last).all(same) {
            let roster: Vec<NetplayRosterEntry> = roster
                .iter()
                .map(|m| NetplayRosterEntry {
                    nick: m.nickname.clone(),
                    role: ipc_role(m.role),
                    ping_ms: m.rtt_ms,
                    relayed: self.socket.is_relayed(m.peer),
                })
                .collect();
            self.stats.last_roster.clone_from(&roster);
            self.outbox.push(NetplayEvent::Lobby {
                role: ipc_role(self.options.role),
                roster,
                experimental: self.options.experimental,
            });
            self.touch(); // waiting-room pings / relay marks
        }
        if *self.lobby.phase() == LobbyPhase::Syncing && !self.lobby.is_host() {
            let percent = (self.lobby.sync_progress() * 10.0).floor() as u8 * 10;
            if self.stats.sync_percent != Some(percent) {
                self.stats.sync_percent = Some(percent);
                self.outbox.push(NetplayEvent::Syncing { percent });
            }
        }
    }

    pub(super) fn track_stall(&mut self, stalled: bool, now: Instant) {
        let since = match (stalled, self.stats.stall_since) {
            (false, _) => None,
            (true, None) => Some(now),
            (true, Some(at)) => Some(at),
        };
        self.stats.stall_since = since;
        let shown = since.is_some_and(|at| now.saturating_duration_since(at) >= STALL_WARNING);
        if shown != self.stats.stalled {
            self.stats.stalled = shown;
            self.touch();
        }
    }

    /// Every 2 s: worst ping, rollbacks of the last second, relay → HUD + event.
    pub(super) fn update_stats(&mut self, now: Instant) {
        if now < self.stats.next_stats {
            return;
        }
        self.stats.next_stats = now + STATS_EVERY;
        let Some(driver) = self.driver.as_mut() else {
            return;
        };
        let stats = driver.stats();
        let ping = stats.peers.iter().filter_map(|p| p.ping_ms).max();
        let roster = self.lobby.roster().iter();
        let relayed = roster
            .filter(|m| self.socket.is_relayed(m.peer) == Some(true))
            .count()
            > 0;
        let rollback_fps = stats.rollback_frames_last_second;
        tracing::debug!(?ping, rollback_fps, relayed, "netplay stats");
        self.stats.ping_ms = ping;
        self.stats.rollback_fps = rollback_fps;
        self.stats.relayed = relayed;
        self.outbox.push(NetplayEvent::Stats {
            ping_ms: ping,
            rollback_fps,
            relayed,
        });
        self.touch();
    }

    /// Overlay repaint key.
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn phase(&self) -> &LobbyPhase {
        self.lobby.phase()
    }

    pub(crate) fn is_host(&self) -> bool {
        self.lobby.is_host()
    }

    pub(crate) fn can_start(&self) -> bool {
        self.lobby.can_start()
    }

    pub(crate) fn sync_progress(&self) -> f32 {
        self.lobby.sync_progress()
    }

    pub(crate) fn room_code(&self) -> Option<&str> {
        self.options.room_code.as_deref()
    }

    pub(crate) fn hud(&self) -> &HudStats {
        &self.stats
    }

    pub(crate) fn roster_lines(&self) -> Vec<RosterLine> {
        let roster = self.lobby.roster().iter();
        roster
            .map(|m| RosterLine {
                nick: m.nickname.clone(),
                role: m.role,
                ping_ms: m.rtt_ms,
            })
            .collect()
    }

    /// Why the session ended (end screen), or a lobby notice.
    pub(crate) fn end_text(&self) -> Option<String> {
        match self.end_reason.as_ref()? {
            EndReason::HostLeft => Some("The host left the game".into()),
            EndReason::Refused(reason) => Some(reason.text()),
            EndReason::Left => None,
            EndReason::Error(message) => Some(format!("Netplay error: {message}")),
        }
    }

    /// Host notice from the lobby (refused joiner, resync not possible).
    pub(crate) fn notice(&self) -> Option<&str> {
        self.lobby.notice()
    }

    /// Whether "<nick> left" is still shown.
    pub(crate) fn peer_left_shown(&self, now: Instant) -> bool {
        let fresh =
            |(_, at): &(String, Instant)| now.saturating_duration_since(*at) < PEER_LEFT_SHOWN;
        self.stats.peer_left.as_ref().is_some_and(fresh)
    }

    /// "<nick> left" while fresh.
    pub(crate) fn peer_left_text(&self, now: Instant) -> Option<String> {
        let (nick, _) = self.stats.peer_left.as_ref()?;
        self.peer_left_shown(now).then(|| format!("{nick} left"))
    }
}
