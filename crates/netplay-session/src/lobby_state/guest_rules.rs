//! Guest side (player / spectator): `Hello` to every new peer, the `Welcome` sender
//! becomes the host (F31), messages from the host only, `Start`, resync (drop the
//! driver, `ResyncReady`), host leave. Sync reception: `guest_sync_rules.rs`.

use super::{EndReason, LobbyEffect, LobbyPhase, LobbyState, StartPlan};
use crate::lobby_protocol::LobbyMessage;
use crate::peer_handles::PeerHandles;
use crate::PeerKey;

impl LobbyState {
    pub(super) fn on_peer_joined(&mut self, peer: PeerKey, out: &mut Vec<LobbyEffect>) {
        self.connected.insert(peer);
        if !self.is_host() && self.phase == LobbyPhase::Handshaking {
            self.send(vec![peer], self.hello(), out);
        }
    }

    pub(super) fn guest_peer_left(&mut self, peer: PeerKey, out: &mut Vec<LobbyEffect>) {
        if self.host == Some(peer) {
            self.end(EndReason::HostLeft, out);
        }
    }

    pub(super) fn on_guest_message(
        &mut self,
        peer: PeerKey,
        msg: LobbyMessage,
        out: &mut Vec<LobbyEffect>,
    ) {
        if self.host.is_none() {
            return self.on_handshake_reply(peer, msg, out);
        }
        if self.host != Some(peer) {
            return; // other guests' Hello and anything else from non-hosts
        }
        match msg {
            LobbyMessage::RosterUpdate { roster } => {
                if self.phase == LobbyPhase::InLobby {
                    let me = roster.iter().find(|m| Some(m.peer) == self.me);
                    self.my_handle = me.and_then(|m| m.handle);
                }
                self.roster = roster;
                out.push(LobbyEffect::NotifyUi);
            }
            LobbyMessage::Refused { reason } => self.end(EndReason::Refused(reason), out),
            LobbyMessage::SyncBegin {
                sram_len,
                rtc_len,
                state_len,
                warmup_frames,
            } => {
                self.on_sync_begin(sram_len, rtc_len, state_len, warmup_frames, out);
            }
            LobbyMessage::SyncChunk {
                kind,
                offset,
                bytes,
            } => self.on_sync_chunk(kind, offset, &bytes, out),
            LobbyMessage::SyncDone { crc32s } => self.on_sync_done(crc32s, out),
            LobbyMessage::Start {
                input_delay,
                epoch,
                players,
                spectators,
            } => {
                if self.phase == LobbyPhase::Starting {
                    self.on_start(input_delay, epoch, &players, &spectators, out);
                }
            }
            LobbyMessage::Resync
                if matches!(self.phase, LobbyPhase::Running | LobbyPhase::Starting) =>
            {
                self.phase = LobbyPhase::Resyncing;
                self.inbox = None;
                out.push(LobbyEffect::DropDriver);
                self.send(vec![peer], LobbyMessage::ResyncReady, out);
                out.push(LobbyEffect::NotifyUi);
            }
            _ => {}
        }
    }

    fn on_handshake_reply(&mut self, peer: PeerKey, msg: LobbyMessage, out: &mut Vec<LobbyEffect>) {
        match msg {
            LobbyMessage::Welcome {
                roster,
                your_handle,
            } if self.phase == LobbyPhase::Handshaking => {
                self.host = Some(peer);
                self.roster = roster;
                self.my_handle = your_handle;
                self.phase = LobbyPhase::InLobby;
                out.push(LobbyEffect::NotifyUi);
            }
            LobbyMessage::Refused { reason } => self.end(EndReason::Refused(reason), out),
            _ => {}
        }
    }

    fn on_start(
        &mut self,
        input_delay: usize,
        epoch: u8,
        players: &[(PeerKey, usize)],
        spectators: &[(PeerKey, usize)],
        out: &mut Vec<LobbyEffect>,
    ) {
        let handles = match PeerHandles::from_start(players, spectators) {
            Ok(handles) if Some(handles.host()) == self.host => handles,
            Ok(_) => return self.end(EndReason::Error("Start names another host".into()), out),
            Err(err) => return self.end(EndReason::Error(err.to_string()), out),
        };
        let me = self.me.unwrap_or(PeerKey(0));
        if handles.handle_of(me).is_none() {
            return self.end(EndReason::Error("not in the session roster".into()), out);
        }
        self.epoch = epoch;
        self.my_handle = handles.handle_of(me);
        let plan = StartPlan {
            handles,
            me,
            role: self.cfg.role,
            input_delay,
            epoch,
        };
        out.push(match self.driver_started {
            true => LobbyEffect::RebuildDriver(plan),
            false => LobbyEffect::StartDriver(plan),
        });
        self.driver_started = true;
        self.phase = LobbyPhase::Running;
        out.push(LobbyEffect::NotifyUi);
    }
}
