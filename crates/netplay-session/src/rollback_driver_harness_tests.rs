//! Test harness: `ScriptedHost` (deterministic counter "game") and peers driven
//! over a `FakeNet` (in-memory, configurable loss / latency).

#![allow(clippy::unwrap_used)]

use std::collections::HashMap;

use ggrs::{Frame, InputStatus};

use super::{RollbackDriver, RollbackHost, TickOutcome};
use crate::desync_checksum::desync_checksum;
use crate::fake_socket::FakeNet;
use crate::lobby_state::StartPlan;
use crate::netplay_input::NetplayInput;
use crate::peer_handles::PeerHandles;
use crate::rollback_config::RollbackConfig;
use crate::room_api_types::SeatRole;
use crate::{NetplayError, PeerKey};

/// State = `u64`; advance = `state = hash(state, inputs)`.
#[derive(Default)]
pub(super) struct ScriptedHost {
    pub state: u64,
    /// State right after advancing frame n (last write wins).
    pub after: HashMap<Frame, u64>,
    /// Every advance per frame: its `is_new` flags in order.
    pub advances: HashMap<Frame, Vec<bool>>,
    /// Flips a bit on every advance of frames ≥ this (a desynced peer).
    pub corrupt_from: Option<Frame>,
}

impl RollbackHost for ScriptedHost {
    fn save(&mut self, _frame: Frame, buf: &mut Vec<u8>) -> Result<u128, NetplayError> {
        buf.extend_from_slice(&self.state.to_le_bytes());
        Ok(desync_checksum(buf, &[]))
    }

    fn load(&mut self, _frame: Frame, state: &[u8]) -> Result<(), NetplayError> {
        let bytes: [u8; 8] = state
            .try_into()
            .map_err(|_| NetplayError::Host("size".into()))?;
        self.state = u64::from_le_bytes(bytes);
        Ok(())
    }

    fn advance(
        &mut self,
        frame: Frame,
        inputs: &[(NetplayInput, InputStatus)],
        is_new: bool,
        _last: bool,
    ) {
        for (input, _) in inputs {
            let mixed = u64::from(input.buttons) ^ (u64::from(input.axes[0] as u16) << 16);
            self.state = (self.state ^ mixed)
                .wrapping_mul(0x0100_0000_01b3)
                .rotate_left(5);
        }
        if self.corrupt_from.is_some_and(|from| frame >= from) {
            self.state ^= 1 << 40;
        }
        self.after.insert(frame, self.state);
        self.advances.entry(frame).or_default().push(is_new);
    }
}

/// Scripted input of `handle` at `frame` (button held 7 frames, phase per player).
pub(super) fn script(frame: Frame, handle: usize) -> NetplayInput {
    let step = (frame.max(0) as usize / 7 + handle * 3) % 11;
    NetplayInput {
        buttons: if step < 10 { 1 << step } else { 0 },
        axes: [(step as i16 - 5) * 1000, 0, 0, 0],
    }
}

pub(super) struct Peer {
    pub key: PeerKey,
    pub driver: RollbackDriver,
    pub host: ScriptedHost,
    pub active: bool,
    pub resims: u64,
    pub stalls: u32,
}

impl Peer {
    pub fn tick(&mut self) -> TickOutcome {
        let local: Vec<NetplayInput> = self
            .plan_handle()
            .map(|h| script(self.driver.current_frame(), h))
            .into_iter()
            .collect();
        let outcome = self.driver.tick(&local, &mut self.host, true).unwrap();
        match outcome {
            TickOutcome::Advanced { resim_frames, .. } => self.resims += u64::from(resim_frames),
            TickOutcome::Stalled => self.stalls += 1,
            TickOutcome::Waiting => {}
        }
        outcome
    }

    fn plan_handle(&self) -> Option<usize> {
        self.driver.local_handles.first().copied()
    }
}

pub(super) const HOST: PeerKey = PeerKey(10);

/// Configs for host + `players` + `spectators` (keys 11.., 20..), `epoch`.
pub(super) fn configs(
    players: usize,
    spectators: usize,
    delay: usize,
    epoch: u8,
) -> Vec<RollbackConfig> {
    let others: Vec<PeerKey> = (0..players).map(|i| PeerKey(11 + i as u128)).collect();
    let specs: Vec<PeerKey> = (0..spectators).map(|i| PeerKey(20 + i as u128)).collect();
    let handles = PeerHandles::assign(HOST, &others, &specs);
    let roles = std::iter::once((HOST, SeatRole::Host))
        .chain(others.iter().map(|&k| (k, SeatRole::Player)))
        .chain(specs.iter().map(|&k| (k, SeatRole::Spectator)));
    roles
        .map(|(me, role)| {
            let plan = StartPlan {
                handles: handles.clone(),
                me,
                role,
                input_delay: delay,
                epoch,
            };
            RollbackConfig::new(plan, 60_000)
        })
        .collect()
}

pub(super) fn peers(net: &FakeNet, configs: &[RollbackConfig]) -> Vec<Peer> {
    configs
        .iter()
        .map(|cfg| Peer {
            key: cfg.plan.me,
            driver: RollbackDriver::new(cfg, net.endpoint(cfg.plan.me)).unwrap(),
            host: ScriptedHost::default(),
            active: true,
            resims: 0,
            stalls: 0,
        })
        .collect()
}

/// Ticks every active peer until `done` (panics after `max` ticks).
pub(super) fn run(net: &FakeNet, peers: &mut [Peer], max: usize, done: impl Fn(&[Peer]) -> bool) {
    for _ in 0..max {
        net.tick();
        if peers.iter().any(|p| p.active && !p.driver.is_running()) {
            // ggrs re-sends a lost sync request only after 200 ms of wall time.
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        for peer in peers.iter_mut().filter(|p| p.active) {
            peer.tick();
        }
        if done(peers) {
            return;
        }
    }
    panic!("not done after {max} ticks");
}

/// Every peer confirmed `frame` (and advanced past it).
pub(super) fn all_confirmed(peers: &[Peer], frame: Frame) -> bool {
    peers
        .iter()
        .all(|p| p.driver.confirmed_frame() >= frame && p.driver.head_frame() > frame)
}
