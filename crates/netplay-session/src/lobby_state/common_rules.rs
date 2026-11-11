//! Rules shared by every role: connect, message dispatch, quit, and the small
//! helpers (own roster line, `Hello`, guest list, send, end).

use std::time::Instant;

use super::{EndReason, LobbyEffect, LobbyPhase, LobbyState};
use crate::lobby_protocol::{LobbyMember, LobbyMessage, RefusalReason};
use crate::{NetplayError, PeerKey};

impl LobbyState {
    pub(super) fn on_connected(&mut self, me: PeerKey, out: &mut Vec<LobbyEffect>) {
        if self.me.is_some() {
            return;
        }
        self.me = Some(me);
        if self.is_host() {
            self.host = Some(me);
            self.my_handle = Some(0);
            self.roster.push(self.own_member(Some(0)));
            self.phase = LobbyPhase::InLobby;
        } else {
            self.phase = LobbyPhase::Handshaking;
            let peers: Vec<PeerKey> = self.connected.iter().copied().collect();
            self.send(peers, self.hello(), out);
        }
        out.push(LobbyEffect::NotifyUi);
    }

    pub(super) fn on_message(
        &mut self,
        peer: PeerKey,
        msg: LobbyMessage,
        now: Instant,
        out: &mut Vec<LobbyEffect>,
    ) {
        match msg {
            LobbyMessage::Ping { nonce } => {
                self.send(vec![peer], LobbyMessage::Pong { nonce }, out)
            }
            LobbyMessage::Leave => self.on_peer_left(peer, out),
            msg if self.is_host() => self.on_host_message(peer, msg, now, out),
            msg => self.on_guest_message(peer, msg, out),
        }
    }

    /// Undecodable packet. Host: an un-greeted sender is refused (Start is not
    /// blocked by it; it ends on the `Refused` it cannot decode either). Guest:
    /// while handshaking only the host (or another joiner) talks to us, so the
    /// error ends the handshake. Anything else is dropped.
    pub(super) fn on_bad_message(
        &mut self,
        peer: PeerKey,
        err: NetplayError,
        out: &mut Vec<LobbyEffect>,
    ) {
        if !self.is_host() {
            if self.phase == LobbyPhase::Handshaking {
                self.end(EndReason::Error(err.to_string()), out);
            }
            return;
        }
        if self.roster.iter().any(|m| m.peer == peer) || !self.refused.insert(peer) {
            return;
        }
        self.notice = Some(format!("A player could not join: {err}"));
        let reason = RefusalReason::Protocol;
        self.send(vec![peer], LobbyMessage::Refused { reason }, out);
        out.push(LobbyEffect::NotifyUi);
    }

    pub(super) fn on_quit(&mut self, out: &mut Vec<LobbyEffect>) {
        let to: Vec<PeerKey> = match (self.is_host(), self.host) {
            (true, _) => self.guests(),
            (false, Some(host)) => vec![host],
            (false, None) => Vec::new(),
        };
        self.send(to, LobbyMessage::Leave, out);
        self.end(EndReason::Left, out);
    }

    pub(super) fn own_member(&self, handle: Option<usize>) -> LobbyMember {
        LobbyMember {
            peer: self.me.unwrap_or(PeerKey(0)),
            nickname: self.cfg.nickname.clone(),
            role: self.cfg.role,
            handle,
            rtt_ms: None,
        }
    }

    pub(super) fn hello(&self) -> LobbyMessage {
        LobbyMessage::Hello {
            fingerprint: self.cfg.fingerprint.clone(),
            nickname: self.cfg.nickname.clone(),
            role: self.cfg.role,
        }
    }

    /// Host: every roster member except the host itself.
    pub(super) fn guests(&self) -> Vec<PeerKey> {
        self.roster
            .iter()
            .map(|m| m.peer)
            .filter(|p| Some(*p) != self.me)
            .collect()
    }

    pub(super) fn send(&self, to: Vec<PeerKey>, msg: LobbyMessage, out: &mut Vec<LobbyEffect>) {
        if !to.is_empty() {
            out.push(LobbyEffect::Send { to, msg });
        }
    }

    pub(super) fn end(&mut self, reason: EndReason, out: &mut Vec<LobbyEffect>) {
        if let (false, Some(host), EndReason::Error(_)) = (self.is_host(), self.host, &reason) {
            self.send(vec![host], LobbyMessage::Leave, out); // host stops waiting for us
        }
        self.phase = LobbyPhase::Ended(reason);
        out.push(LobbyEffect::NotifyUi);
    }
}
