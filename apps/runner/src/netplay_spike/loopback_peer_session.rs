//! One ggrs `P2PSession` of the loopback peer on the shared ggrs channel: ~60 Hz
//! ticks, scripted input per handle, until `confirmed_frame() >= frames`, then a 2 s
//! settle that keeps acking the peer.

use std::time::{Duration, Instant};

use anyhow::Context;
use ggrs::{GgrsError, GgrsEvent, P2PSession, PlayerType, SessionBuilder, SessionState};
use libretro_host::CoreSession;
use matchbox_socket::{PeerId, WebRtcSocket};

use super::core_rollback_game::CoreRollbackGame;
use super::shared_ggrs_channel::{SharedChannel, SharedGgrsChannel};
use super::spike_config::{scripted_input, SpikeConfig};

type Cfg = SpikeConfig<PeerId>;
const TICK: Duration = Duration::from_micros(16_667);
const SETTLE: Duration = Duration::from_secs(2);

/// What every session of one peer shares.
pub(crate) struct PeerRun<'a> {
    pub socket: &'a mut WebRtcSocket,
    pub players: &'a [PlayerType<PeerId>],
    pub local: usize,
    pub frames: i32,
    pub channel: SharedChannel,
}

/// `NOSTYRA_SPIKE_INPUT_DELAY` (default 2): 0 forces rollbacks on loopback.
fn input_delay() -> usize {
    let value = std::env::var("NOSTYRA_SPIKE_INPUT_DELAY").ok();
    value.and_then(|v| v.parse().ok()).unwrap_or(2)
}

fn build(run: &PeerRun<'_>, epoch: u8) -> anyhow::Result<P2PSession<Cfg>> {
    let mut builder = SessionBuilder::<Cfg>::new()
        .with_num_players(run.players.len())?
        .with_input_delay(input_delay())
        .with_desync_detection_mode(ggrs::DesyncDetection::On { interval: 60 });
    for (handle, player) in run.players.iter().enumerate() {
        builder = builder.add_player(*player, handle)?;
    }
    let socket = SharedGgrsChannel::new(run.channel.clone(), epoch);
    Ok(builder.start_p2p_session(socket)?)
}

/// Plays one session from the core's current state; prints the `LOOPBACK` result.
pub(crate) fn play_session(
    run: &mut PeerRun<'_>,
    core: &mut CoreSession,
    epoch: u8,
) -> anyhow::Result<()> {
    let mut session = build(run, epoch)?;
    let mut game = CoreRollbackGame::default();
    let (started, mut next, mut done_at, mut events) =
        (Instant::now(), Instant::now(), None, Vec::new());
    let mut synced_ms = None;
    while done_at.is_none_or(|at: Instant| at.elapsed() < SETTLE) {
        anyhow::ensure!(
            started.elapsed() < Duration::from_secs(120),
            "session {epoch} timed out"
        );
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
        next += TICK;
        for (_, state) in run.socket.update_peers() {
            events.push(match state {
                matchbox_socket::PeerState::Connected => "PeerConnected",
                matchbox_socket::PeerState::Disconnected => "PeerDisconnected",
            });
        }
        session.poll_remote_clients();
        events.extend(session.events().map(|e| event_name(&e)));
        if session.current_state() != SessionState::Running {
            continue;
        }
        synced_ms.get_or_insert(started.elapsed().as_millis());
        let frame = session.current_frame();
        session.add_local_input(run.local, scripted_input(frame, run.local))?;
        match session.advance_frame() {
            Ok(requests) => game.handle(core, requests)?,
            Err(GgrsError::PredictionThreshold) => {}
            Err(error) => return Err(error.into()),
        }
        if done_at.is_none() && session.confirmed_frame() >= run.frames {
            done_at = Some(Instant::now());
        }
    }
    report(run, &session, &game, epoch, synced_ms, &events)
}

fn report(
    run: &PeerRun<'_>,
    session: &P2PSession<Cfg>,
    game: &CoreRollbackGame,
    epoch: u8,
    synced_ms: Option<u128>,
    events: &[&'static str],
) -> anyhow::Result<()> {
    let remote = session
        .remote_player_handles()
        .first()
        .copied()
        .context("no remote")?;
    let stats = session.network_stats(remote).ok();
    let peer = run.players.iter().find_map(|p| match p {
        PlayerType::Remote(id) => Some(*id),
        _ => None,
    });
    let relayed = peer.and_then(|id| run.socket.is_relayed(id));
    let (once, more) = game.advance_histogram();
    let crc = game
        .crc_after
        .get(&run.frames)
        .context("no crc for frame N")?;
    let mut kinds: Vec<_> = events.to_vec();
    kinds.dedup();
    eprintln!(
        "LOOPBACK epoch={epoch} handle={} ram_crc={crc:08x} confirmed={} current={} synced_ms={synced_ms:?} \
         ping_ms={:?} frames_behind={:?} relayed={relayed:?} advanced_once={once} advanced_more={more} \
         loads={} source={:?} events={kinds:?}",
        run.local,
        session.confirmed_frame(),
        session.current_frame(),
        stats.map(|s| s.ping),
        stats.map(|s| s.local_frames_behind),
        game.loads,
        game.source,
    );
    Ok(())
}

fn event_name(event: &GgrsEvent<Cfg>) -> &'static str {
    match event {
        GgrsEvent::Synchronizing { .. } => "Synchronizing",
        GgrsEvent::Synchronized { .. } => "Synchronized",
        GgrsEvent::Disconnected { .. } => "Disconnected",
        GgrsEvent::NetworkInterrupted { .. } => "NetworkInterrupted",
        GgrsEvent::NetworkResumed { .. } => "NetworkResumed",
        GgrsEvent::WaitRecommendation { .. } => "WaitRecommendation",
        GgrsEvent::DesyncDetected { .. } => "DesyncDetected",
    }
}
