#![allow(clippy::unwrap_used)]

use super::*;

fn payload() -> SyncPayload {
    SyncPayload {
        sram: (0..8192u32).map(|i| (i % 251) as u8).collect(),
        rtc: Some(vec![1, 2, 3, 4]),
        state: (0..3_000_000u32).map(|i| (i * 7 % 253) as u8).collect(),
    }
}

/// Splits the sequence into (begin lengths, chunks, crcs).
type Lens = (u32, Option<u32>, u32);
type Chunks = Vec<(SyncKind, u32, Vec<u8>)>;

fn parts(messages: Vec<LobbyMessage>) -> (Lens, Chunks, SyncCrcs) {
    let mut iter = messages.into_iter();
    let Some(LobbyMessage::SyncBegin {
        sram_len,
        rtc_len,
        state_len,
        warmup_frames: 2,
    }) = iter.next()
    else {
        panic!("SyncBegin first")
    };
    let mut chunks = Vec::new();
    let mut crcs = None;
    for msg in iter {
        match msg {
            LobbyMessage::SyncChunk {
                kind,
                offset,
                bytes,
            } => chunks.push((kind, offset, bytes)),
            LobbyMessage::SyncDone { crc32s } => crcs = Some(crc32s),
            other => panic!("unexpected {other:?}"),
        }
    }
    ((sram_len, rtc_len, state_len), chunks, crcs.unwrap())
}

#[test]
fn three_megabytes_round_trip_in_reverse_order() {
    let sent = payload();
    let ((sram, rtc, state), mut chunks, crcs) = parts(outgoing_sync(&sent, 2).unwrap());
    assert!(chunks.iter().all(|(_, _, b)| b.len() <= SYNC_CHUNK_BYTES));
    assert_eq!(
        chunks.len(),
        1 + 1 + 3_000_000usize.div_ceil(SYNC_CHUNK_BYTES)
    );
    chunks.reverse();
    let mut inbox = SyncInbox::new(sram, rtc, state).unwrap();
    for (i, (kind, offset, bytes)) in chunks.iter().enumerate() {
        inbox.accept(*kind, *offset, bytes).unwrap();
        if i == 0 {
            assert!(inbox.progress() > 0.0 && inbox.progress() < 0.01);
        }
        inbox.accept(*kind, *offset, bytes).unwrap(); // duplicate ignored
    }
    assert!((inbox.progress() - 1.0).abs() < f32::EPSILON);
    assert_eq!(inbox.finish(crcs).unwrap(), sent);
}

#[test]
fn crc_mismatch_and_missing_chunks_are_errors() {
    let sent = payload();
    let ((sram, rtc, state), chunks, crcs) = parts(outgoing_sync(&sent, 2).unwrap());
    let mut inbox = SyncInbox::new(sram, rtc, state).unwrap();
    for (kind, offset, bytes) in &chunks {
        let mut bytes = bytes.clone();
        if *kind == SyncKind::State && *offset == 0 {
            bytes[0] ^= 1;
        }
        inbox.accept(*kind, *offset, &bytes).unwrap();
    }
    assert_eq!(
        inbox.finish(crcs),
        Err(NetplayError::Sync("crc mismatch".into()))
    );
    let mut partial = SyncInbox::new(sram, rtc, state).unwrap();
    for (kind, offset, bytes) in chunks.iter().skip(1) {
        partial.accept(*kind, *offset, bytes).unwrap();
    }
    assert_eq!(
        partial.finish(crcs),
        Err(NetplayError::Sync("missing chunks".into()))
    );
}

#[test]
fn bad_chunks_and_sizes_are_refused() {
    let mut inbox = SyncInbox::new(10, None, 20).unwrap();
    assert!(
        inbox.accept(SyncKind::Rtc, 0, &[0; 4]).is_err(),
        "rtc not announced"
    );
    assert!(
        inbox.accept(SyncKind::Sram, 0, &[0; 11]).is_err(),
        "too long"
    );
    assert!(
        inbox.accept(SyncKind::State, 16384, &[0; 4]).is_err(),
        "outside"
    );
    assert!(SyncInbox::new(0, None, 0).is_err(), "state is never empty");
    let cap = MAX_SYNC_BYTES as u32;
    assert!(SyncInbox::new(1, None, cap).is_err(), "over 64 MiB");
    assert!(SyncInbox::new(0, None, cap).is_ok());
    let empty_state = SyncPayload::default();
    assert!(outgoing_sync(&empty_state, 0).is_err());
}

#[test]
fn empty_sram_and_no_rtc_still_complete() {
    let sent = SyncPayload {
        sram: vec![],
        rtc: None,
        state: vec![9; 5],
    };
    let messages = outgoing_sync(&sent, 2).unwrap();
    let ((sram, rtc, state), chunks, crcs) = parts(messages);
    let mut inbox = SyncInbox::new(sram, rtc, state).unwrap();
    for (kind, offset, bytes) in &chunks {
        inbox.accept(*kind, *offset, bytes).unwrap();
    }
    assert_eq!(inbox.finish(crcs).unwrap(), sent);
}
