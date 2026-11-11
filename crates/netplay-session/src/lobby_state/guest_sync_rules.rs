//! Guest side, sync reception: sizes checked against our own (warm-up frames,
//! SRAM, RTC: F18 → refuse + `Leave`), the warm-up runs before the first load,
//! chunks reassembled, crc verified, then `LoadSync` → `SyncLoaded` → `SyncAck`.

use super::{EndReason, LobbyEffect, LobbyPhase, LobbyState};
use crate::lobby_protocol::{LobbyMessage, RefusalReason, SyncCrcs, SyncKind};
use crate::sync_transfer::SyncInbox;

impl LobbyState {
    pub(super) fn on_sync_begin(
        &mut self,
        sram_len: u32,
        rtc_len: Option<u32>,
        state_len: u32,
        warmup: u32,
        out: &mut Vec<LobbyEffect>,
    ) {
        if !matches!(self.phase, LobbyPhase::InLobby | LobbyPhase::Resyncing) {
            return;
        }
        let ours = self.cfg.fingerprint.warmup_frames;
        let refusal = if warmup != ours {
            Some(RefusalReason::WarmupFrames {
                host: warmup,
                guest: ours,
            })
        } else if sram_len != self.cfg.sram_len {
            Some(RefusalReason::SramSize {
                host: sram_len,
                guest: self.cfg.sram_len,
            })
        } else if rtc_len != self.cfg.rtc_len {
            Some(RefusalReason::RtcMismatch)
        } else {
            None
        };
        if let Some(reason) = refusal {
            self.send(self.host.into_iter().collect(), LobbyMessage::Leave, out);
            return self.end(EndReason::Refused(reason), out);
        }
        match SyncInbox::new(sram_len, rtc_len, state_len) {
            Ok(inbox) => self.inbox = Some(inbox),
            Err(err) => return self.end(EndReason::Error(err.to_string()), out),
        }
        // Warm-up only before the first session: on resync the core is already past it.
        if warmup > 0 && !self.driver_started {
            out.push(LobbyEffect::RunWarmup(warmup));
        }
        self.phase = LobbyPhase::Syncing;
        self.sync_progress = 0.0;
        out.push(LobbyEffect::NotifyUi);
    }

    pub(super) fn on_sync_chunk(
        &mut self,
        kind: SyncKind,
        offset: u32,
        bytes: &[u8],
        out: &mut Vec<LobbyEffect>,
    ) {
        let Some(inbox) = self.inbox.as_mut() else {
            return;
        };
        match inbox.accept(kind, offset, bytes) {
            Ok(()) => self.sync_progress = inbox.progress(),
            Err(err) => self.end(EndReason::Error(err.to_string()), out),
        }
    }

    pub(super) fn on_sync_done(&mut self, crcs: SyncCrcs, out: &mut Vec<LobbyEffect>) {
        let Some(inbox) = self.inbox.take() else {
            return;
        };
        match inbox.finish(crcs) {
            Ok(payload) => out.push(LobbyEffect::LoadSync(payload)),
            Err(err) => self.end(EndReason::Error(err.to_string()), out),
        }
    }

    pub(super) fn on_sync_loaded(&mut self, out: &mut Vec<LobbyEffect>) {
        if self.phase != LobbyPhase::Syncing || self.inbox.is_some() {
            return;
        }
        self.send(self.host.into_iter().collect(), LobbyMessage::SyncAck, out);
        self.phase = LobbyPhase::Starting;
        out.push(LobbyEffect::NotifyUi);
    }
}
