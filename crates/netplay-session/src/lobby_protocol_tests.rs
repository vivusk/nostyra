#![allow(clippy::unwrap_used)]

use super::*;
use crate::session_fingerprint::tests::sample as fingerprint;
use crate::session_fingerprint::FingerprintField;

fn every_message() -> Vec<LobbyMessage> {
    let member = LobbyMember {
        peer: PeerKey(7),
        nickname: "Ann".into(),
        role: SeatRole::Player,
        handle: Some(1),
        rtt_ms: Some(42),
    };
    let mismatch = FingerprintMismatch {
        field: FingerprintField::CoreVersion,
        ours: "1.62".into(),
        theirs: "1.63".into(),
    };
    vec![
        LobbyMessage::Hello {
            fingerprint: fingerprint(),
            nickname: "Bob".into(),
            role: SeatRole::Spectator,
        },
        LobbyMessage::Welcome {
            roster: vec![member.clone()],
            your_handle: Some(1),
        },
        LobbyMessage::Refused {
            reason: RefusalReason::Mismatch(vec![mismatch]),
        },
        LobbyMessage::Refused {
            reason: RefusalReason::Started,
        },
        LobbyMessage::Refused {
            reason: RefusalReason::RoomFull,
        },
        LobbyMessage::Refused {
            reason: RefusalReason::SramSize {
                host: 8192,
                guest: 0,
            },
        },
        LobbyMessage::Refused {
            reason: RefusalReason::RtcMismatch,
        },
        LobbyMessage::Refused {
            reason: RefusalReason::WarmupFrames { host: 2, guest: 0 },
        },
        LobbyMessage::RosterUpdate {
            roster: vec![member],
        },
        LobbyMessage::SyncBegin {
            sram_len: 8192,
            rtc_len: Some(16),
            state_len: 1 << 20,
            warmup_frames: 3,
        },
        LobbyMessage::SyncChunk {
            kind: SyncKind::State,
            offset: 16384,
            bytes: vec![0xAB; SYNC_CHUNK_BYTES],
        },
        LobbyMessage::SyncDone {
            crc32s: SyncCrcs {
                sram: 1,
                rtc: None,
                state: 3,
            },
        },
        LobbyMessage::SyncAck,
        LobbyMessage::Start {
            input_delay: 2,
            epoch: 1,
            players: vec![(PeerKey(1), 0), (PeerKey(9), 1)],
            spectators: vec![(PeerKey(5), 2)],
        },
        LobbyMessage::Resync,
        LobbyMessage::ResyncReady,
        LobbyMessage::Leave,
        LobbyMessage::Ping { nonce: u64::MAX },
        LobbyMessage::Pong { nonce: 3 },
    ]
}

#[test]
fn every_message_round_trips() {
    for msg in every_message() {
        let packet = encode_lobby(&msg).unwrap();
        assert!(packet.len() <= MAX_LOBBY_MESSAGE_BYTES);
        assert_eq!(decode_lobby(&packet).unwrap(), msg);
    }
}

#[test]
fn unknown_version_tag_is_a_protocol_version_error() {
    let mut packet = encode_lobby(&LobbyMessage::SyncAck).unwrap().to_vec();
    packet[0] = 99;
    let err = decode_lobby(&packet).unwrap_err();
    assert_eq!(
        err,
        NetplayError::ProtocolVersion {
            found: 99,
            expected: LOBBY_PROTOCOL_VERSION
        }
    );
}

#[test]
fn garbage_short_and_trailing_bytes_are_refused() {
    assert!(matches!(decode_lobby(&[1]), Err(NetplayError::Protocol(_))));
    let mut packet = encode_lobby(&LobbyMessage::Leave).unwrap().to_vec();
    packet.push(0);
    assert!(matches!(
        decode_lobby(&packet),
        Err(NetplayError::Protocol(_))
    ));
    let mut junk = LOBBY_PROTOCOL_VERSION.to_le_bytes().to_vec();
    junk.extend_from_slice(&[250, 250, 250, 250]);
    assert!(matches!(
        decode_lobby(&junk),
        Err(NetplayError::Protocol(_))
    ));
}

#[test]
fn refusal_texts_name_the_reason() {
    assert_eq!(RefusalReason::Started.text(), "Game already started");
    let diff = vec![FingerprintMismatch {
        field: FingerprintField::CoreVersion,
        ours: "1.62".into(),
        theirs: "1.63".into(),
    }];
    let text = RefusalReason::Mismatch(diff).text();
    assert_eq!(text, "Different core version: 1.62 vs 1.63");
}
