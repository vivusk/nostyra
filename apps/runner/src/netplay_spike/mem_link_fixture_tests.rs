//! In-memory delayed ggrs link for the F20 / F23 tests: two `P2PSession`s over a
//! shared queue (3-tick one-way delay, per-packet session epoch) and a fake game that
//! hashes every input and counts advances per frame.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use ggrs::{GgrsError, GgrsRequest, InputStatus, Message, NonBlockingSocket, P2PSession};
use ggrs::{PlayerType, SessionBuilder, SessionState};

use super::spike_config::{scripted_input, SpikeConfig};

pub(super) type Cfg = SpikeConfig<u8>;
/// (deliver at tick, epoch, message) towards one peer.
type Wire = Rc<RefCell<VecDeque<(u32, u8, Message)>>>;

/// One end of the in-memory link; `epoch` tags sent packets, `filter` drops others.
struct MemSocket {
    out: Wire,
    inbox: Wire,
    peer: u8,
    clock: Rc<Cell<u32>>,
    epoch: u8,
    filter: bool,
}

impl NonBlockingSocket<u8> for MemSocket {
    fn send_to(&mut self, msg: &Message, _addr: &u8) {
        let at = self.clock.get() + 3; // 3 ticks one-way delay
        self.out
            .borrow_mut()
            .push_back((at, self.epoch, msg.clone()));
    }

    fn receive_all_messages(&mut self) -> Vec<(u8, Message)> {
        let now = self.clock.get();
        let mut inbox = self.inbox.borrow_mut();
        let mut out = Vec::new();
        while inbox.front().is_some_and(|(at, _, _)| *at <= now) {
            let (_, epoch, msg) = inbox.pop_front().unwrap();
            if !self.filter || epoch == self.epoch {
                out.push((self.peer, msg));
            }
        }
        out
    }
}

/// Fake game: a hash of every input; counts advances per frame.
#[derive(Default)]
pub(super) struct Game {
    pub(super) hash: u64,
    pub(super) frame: i32,
    pub(super) advances: Vec<u8>,
    pub(super) predicted_single: usize,
    statuses: Vec<bool>,
    /// Hash right after producing frame n (last write wins).
    pub(super) after: std::collections::HashMap<i32, u64>,
}

impl Game {
    fn handle(&mut self, requests: Vec<GgrsRequest<Cfg>>) {
        for request in requests {
            match request {
                GgrsRequest::SaveGameState { cell, frame } => {
                    cell.save(
                        frame,
                        Some((Vec::new(), u128::from(self.hash))),
                        Some(u128::from(self.hash)),
                    );
                }
                GgrsRequest::LoadGameState { cell, frame } => {
                    self.hash = cell.load().unwrap().1 as u64;
                    self.frame = frame;
                }
                GgrsRequest::AdvanceFrame { inputs } => {
                    for (input, _) in &inputs {
                        self.hash =
                            (self.hash ^ u64::from(input.buttons)).wrapping_mul(0x0100_0000_01b3);
                    }
                    let predicted = inputs.iter().any(|(_, s)| *s == InputStatus::Predicted);
                    let i = self.frame as usize;
                    if self.advances.len() <= i {
                        self.advances.resize(i + 1, 0);
                        self.statuses.resize(i + 1, false);
                    }
                    self.advances[i] += 1;
                    self.statuses[i] = predicted;
                    self.frame += 1;
                    self.after.insert(self.frame, self.hash);
                }
            }
        }
    }

    pub(super) fn report(&mut self) -> (usize, usize) {
        let once = self.advances.iter().filter(|&&n| n == 1).count();
        self.predicted_single = (0..self.advances.len())
            .filter(|&i| self.advances[i] == 1 && self.statuses[i])
            .count();
        (once, self.advances.len() - once)
    }
}

pub(super) struct Link {
    a_to_b: Wire,
    b_to_a: Wire,
    pub(super) clock: Rc<Cell<u32>>,
}

pub(super) fn session(link: &Link, me: usize, epoch: u8, filter: bool) -> P2PSession<Cfg> {
    let (out, inbox) = match me {
        0 => (link.a_to_b.clone(), link.b_to_a.clone()),
        _ => (link.b_to_a.clone(), link.a_to_b.clone()),
    };
    let peer = (1 - me) as u8;
    let socket = MemSocket {
        out,
        inbox,
        peer,
        clock: link.clock.clone(),
        epoch,
        filter,
    };
    SessionBuilder::<Cfg>::new()
        .with_num_players(2)
        .unwrap()
        .add_player(PlayerType::Local, me)
        .unwrap()
        .add_player(PlayerType::Remote(peer), 1 - me)
        .unwrap()
        .start_p2p_session(socket)
        .unwrap()
}

/// One tick of one peer: poll, and advance once when running.
pub(super) fn tick(s: &mut P2PSession<Cfg>, game: &mut Game, me: usize) {
    s.poll_remote_clients();
    if s.current_state() != SessionState::Running {
        return;
    }
    s.add_local_input(me, scripted_input(s.current_frame(), me))
        .unwrap();
    match s.advance_frame() {
        Ok(requests) => game.handle(requests),
        Err(GgrsError::PredictionThreshold) => {}
        Err(error) => panic!("{error}"),
    }
}

pub(super) fn link() -> Link {
    Link {
        a_to_b: Wire::default(),
        b_to_a: Wire::default(),
        clock: Rc::default(),
    }
}

/// Runs both peers until each confirmed `frames`; returns the games.
pub(super) fn play(
    link: &Link,
    a: &mut P2PSession<Cfg>,
    b: &mut P2PSession<Cfg>,
    frames: i32,
) -> (Game, Game) {
    let (mut ga, mut gb) = (Game::default(), Game::default());
    for _ in 0..20_000 {
        link.clock.set(link.clock.get() + 1);
        if a.current_state() != SessionState::Running || b.current_state() != SessionState::Running
        {
            // ggrs re-sends a lost sync request only after 200 ms of wall time.
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        tick(a, &mut ga, 0);
        tick(b, &mut gb, 1);
        let done = |s: &P2PSession<Cfg>| {
            s.current_state() == SessionState::Running && s.confirmed_frame() >= frames
        };
        if done(a) && done(b) && ga.frame > frames && gb.frame > frames {
            break;
        }
    }
    (ga, gb)
}
