#![allow(clippy::unwrap_used)]

use ggrs::{GameStateCell, InputStatus};

use super::*;
use crate::netplay_input::NetplayInput;

/// Records (frame, is_new, last_of_tick) per advance; state = frame number.
#[derive(Default)]
struct Recorder {
    state: i32,
    advances: Vec<(Frame, bool, bool)>,
}

impl RollbackHost for Recorder {
    fn save(&mut self, _frame: Frame, buf: &mut Vec<u8>) -> Result<u128, NetplayError> {
        buf.extend_from_slice(&self.state.to_le_bytes());
        Ok(self.state as u128)
    }

    fn load(&mut self, _frame: Frame, state: &[u8]) -> Result<(), NetplayError> {
        self.state = i32::from_le_bytes(state.try_into().unwrap());
        Ok(())
    }

    fn advance(
        &mut self,
        frame: Frame,
        _: &[(NetplayInput, InputStatus)],
        is_new: bool,
        last: bool,
    ) {
        self.state += 1;
        self.advances.push((frame, is_new, last));
    }
}

fn advance() -> GgrsRequest<NetplayConfig> {
    GgrsRequest::AdvanceFrame {
        inputs: vec![(NetplayInput::default(), InputStatus::Confirmed); 2],
    }
}

fn save(cell: &GameStateCell<Frame>, frame: Frame) -> GgrsRequest<NetplayConfig> {
    GgrsRequest::SaveGameState {
        cell: cell.clone(),
        frame,
    }
}

/// Frames 0..=5 advanced (head 5), cells kept for loads.
fn warmed() -> (FrameCursor, StateRing, Recorder, Vec<GameStateCell<Frame>>) {
    let (mut cursor, mut ring, mut host) =
        (FrameCursor::START, StateRing::new(10), Recorder::default());
    let cells: Vec<GameStateCell<Frame>> = (0..10).map(|_| GameStateCell::default()).collect();
    for frame in 0..=5 {
        let requests = vec![save(&cells[frame as usize], frame), advance()];
        apply_requests(requests, &mut cursor, &mut ring, &mut host, true).unwrap();
    }
    (cursor, ring, host, cells)
}

#[test]
fn last_of_tick_marks_only_the_final_advance() {
    let (mut cursor, mut ring, mut host, cells) = warmed();
    assert_eq!(cursor.head, 5);
    host.advances.clear();
    // Roll back to 5, re-simulate it, then two new frames (spectator-like catch-up).
    let requests = vec![
        GgrsRequest::LoadGameState {
            cell: cells[5].clone(),
            frame: 5,
        },
        advance(),
        save(&cells[6], 6),
        advance(),
        save(&cells[7], 7),
        advance(),
    ];
    let applied = apply_requests(requests, &mut cursor, &mut ring, &mut host, true).unwrap();
    assert_eq!(
        host.advances,
        vec![(5, false, false), (6, true, false), (7, true, true)]
    );
    assert_eq!(
        applied,
        Applied {
            new_frames: 2,
            resim_frames: 1
        }
    );
    assert_eq!(host.state, 8, "loaded state of frame 5 (= 5) + 3 advances");
    let more = vec![save(&cells[8], 8), advance()];
    apply_requests(more, &mut cursor, &mut ring, &mut host, false).unwrap();
    assert_eq!(
        host.advances.last(),
        Some(&(8, true, false)),
        "last_due = false → all false"
    );
}

#[test]
fn ring_reuses_buffers_and_refuses_lost_frames() {
    let (mut cursor, mut ring, mut host, cells) = warmed();
    for frame in 6..200 {
        let requests = vec![save(&cells[frame as usize % 10], frame), advance()];
        apply_requests(requests, &mut cursor, &mut ring, &mut host, true).unwrap();
    }
    assert_eq!(ring.saves, 200);
    assert!(
        ring.grows <= 10,
        "only the first fill of each slot grows: {}",
        ring.grows
    );
    let stale = vec![GgrsRequest::LoadGameState {
        cell: cells[3].clone(),
        frame: 3,
    }];
    let err = apply_requests(stale, &mut cursor, &mut ring, &mut host, true).unwrap_err();
    assert!(matches!(err, NetplayError::Rollback(_)));
}

#[test]
fn reset_forgets_frames_but_keeps_buffers() {
    let (_, mut ring, mut host, cells) = warmed();
    ring.reset(10);
    let mut cursor = FrameCursor::START;
    let load = vec![GgrsRequest::LoadGameState {
        cell: cells[2].clone(),
        frame: 2,
    }];
    assert!(apply_requests(load, &mut cursor, &mut ring, &mut host, true).is_err());
    assert_eq!((ring.saves, ring.grows), (0, 0));
}
