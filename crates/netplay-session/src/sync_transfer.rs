//! Host → guest transfer of SRAM, RTC and the start state over the reliable lobby
//! channel: [`outgoing_sync`] chunks a [`SyncPayload`] into `SyncBegin`, 16 KiB
//! `SyncChunk`s and `SyncDone`; [`SyncInbox`] reassembles chunks in any order,
//! reports progress and verifies each blob's crc32. Total size capped at 64 MiB.

use crate::lobby_protocol::{LobbyMessage, SyncCrcs, SyncKind, SYNC_CHUNK_BYTES};
use crate::NetplayError;

/// Largest transfer accepted (all blobs together).
pub const MAX_SYNC_BYTES: u64 = 64 * 1024 * 1024;

/// What the host sends at `Start` (and on resync).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncPayload {
    pub sram: Vec<u8>,
    /// `None` when the core exposes no RTC memory.
    pub rtc: Option<Vec<u8>>,
    /// `Normal`-context full state; never empty.
    pub state: Vec<u8>,
}

fn crc(bytes: &[u8]) -> u32 {
    crc32fast::hash(bytes)
}

fn len32(bytes: &[u8]) -> Result<u32, NetplayError> {
    u32::try_from(bytes.len()).map_err(|_| sync_error("blob over 4 GiB"))
}

fn sync_error(text: &str) -> NetplayError {
    NetplayError::Sync(text.to_owned())
}

/// The whole message sequence for one guest.
pub fn outgoing_sync(
    payload: &SyncPayload,
    warmup_frames: u32,
) -> Result<Vec<LobbyMessage>, NetplayError> {
    let rtc_len = payload.rtc.as_deref().map(len32).transpose()?;
    let (sram_len, state_len) = (len32(&payload.sram)?, len32(&payload.state)?);
    check_sizes(sram_len, rtc_len, state_len)?;
    let mut out = vec![LobbyMessage::SyncBegin {
        sram_len,
        rtc_len,
        state_len,
        warmup_frames,
    }];
    let blobs = [
        (SyncKind::Sram, Some(payload.sram.as_slice())),
        (SyncKind::Rtc, payload.rtc.as_deref()),
        (SyncKind::State, Some(payload.state.as_slice())),
    ];
    for (kind, blob) in blobs {
        for (index, chunk) in blob
            .unwrap_or_default()
            .chunks(SYNC_CHUNK_BYTES)
            .enumerate()
        {
            out.push(LobbyMessage::SyncChunk {
                kind,
                offset: (index * SYNC_CHUNK_BYTES) as u32,
                bytes: chunk.to_vec(),
            });
        }
    }
    out.push(LobbyMessage::SyncDone {
        crc32s: SyncCrcs {
            sram: crc(&payload.sram),
            rtc: payload.rtc.as_deref().map(crc),
            state: crc(&payload.state),
        },
    });
    Ok(out)
}

fn check_sizes(sram: u32, rtc: Option<u32>, state: u32) -> Result<(), NetplayError> {
    if state == 0 {
        return Err(sync_error("empty state"));
    }
    let total = u64::from(sram) + u64::from(rtc.unwrap_or(0)) + u64::from(state);
    if total > MAX_SYNC_BYTES {
        return Err(sync_error("transfer over 64 MiB"));
    }
    Ok(())
}

/// One blob being reassembled.
struct Blob {
    bytes: Vec<u8>,
    got: Vec<bool>,
}

impl Blob {
    fn new(len: u32) -> Self {
        let len = len as usize;
        Self {
            bytes: vec![0; len],
            got: vec![false; len.div_ceil(SYNC_CHUNK_BYTES)],
        }
    }

    fn accept(&mut self, offset: u32, chunk: &[u8]) -> Result<bool, NetplayError> {
        let offset = offset as usize;
        let index = offset / SYNC_CHUNK_BYTES;
        let expected = SYNC_CHUNK_BYTES.min(self.bytes.len().saturating_sub(offset));
        if !offset.is_multiple_of(SYNC_CHUNK_BYTES)
            || index >= self.got.len()
            || chunk.len() != expected
        {
            return Err(sync_error("chunk outside the announced blob"));
        }
        if std::mem::replace(&mut self.got[index], true) {
            return Ok(false); // duplicate: ignore
        }
        self.bytes[offset..offset + chunk.len()].copy_from_slice(chunk);
        Ok(true)
    }

    fn complete(&self) -> bool {
        self.got.iter().all(|&got| got)
    }
}

/// Guest-side reassembly of one transfer (from `SyncBegin` to `SyncDone`).
pub struct SyncInbox {
    sram: Blob,
    rtc: Option<Blob>,
    state: Blob,
    total: u64,
    received: u64,
}

impl SyncInbox {
    /// Starts a transfer announced by `SyncBegin`.
    pub fn new(sram_len: u32, rtc_len: Option<u32>, state_len: u32) -> Result<Self, NetplayError> {
        check_sizes(sram_len, rtc_len, state_len)?;
        let total = u64::from(sram_len) + u64::from(rtc_len.unwrap_or(0)) + u64::from(state_len);
        Ok(Self {
            sram: Blob::new(sram_len),
            rtc: rtc_len.map(Blob::new),
            state: Blob::new(state_len),
            total,
            received: 0,
        })
    }

    /// Stores one chunk (any order; duplicates ignored).
    pub fn accept(
        &mut self,
        kind: SyncKind,
        offset: u32,
        bytes: &[u8],
    ) -> Result<(), NetplayError> {
        let blob = match kind {
            SyncKind::Sram => &mut self.sram,
            SyncKind::State => &mut self.state,
            SyncKind::Rtc => self
                .rtc
                .as_mut()
                .ok_or_else(|| sync_error("rtc not announced"))?,
        };
        if blob.accept(offset, bytes)? {
            self.received += bytes.len() as u64;
        }
        Ok(())
    }

    /// 0.0 ..= 1.0 for the waiting-room bar.
    pub fn progress(&self) -> f32 {
        (self.received as f64 / self.total as f64) as f32
    }

    /// Completes the transfer: every chunk present and every crc right.
    pub fn finish(self, crcs: SyncCrcs) -> Result<SyncPayload, NetplayError> {
        let complete = self.sram.complete()
            && self.state.complete()
            && self.rtc.as_ref().is_none_or(Blob::complete);
        if !complete {
            return Err(sync_error("missing chunks"));
        }
        let rtc = self.rtc.map(|blob| blob.bytes);
        let crc_ok = crc(&self.sram.bytes) == crcs.sram
            && crc(&self.state.bytes) == crcs.state
            && rtc.as_deref().map(crc) == crcs.rtc;
        if !crc_ok {
            return Err(sync_error("crc mismatch"));
        }
        Ok(SyncPayload {
            sram: self.sram.bytes,
            rtc,
            state: self.state.bytes,
        })
    }
}

#[cfg(test)]
#[path = "sync_transfer_tests.rs"]
mod tests;
