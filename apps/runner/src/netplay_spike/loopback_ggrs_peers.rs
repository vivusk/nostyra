//! Phase-00 T4: `nostyra-runner --netplay-spike-peer <room-url> <core-id> <core-path>
//! <rom> <data-dir> [frames]`. Joins a stock `matchbox_server` room, waits for one
//! peer, swaps a 16 KiB blob on the reliable channel, then plays `frames` ggrs frames
//! twice on the same unreliable channel (session epochs 1 and 2, both from the boot
//! state) and prints `LOOPBACK` lines (crc of confirmed frame N, ping, relayed,
//! re-advance counts). Never prints peer ids, room urls or ICE data.

use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use libretro_host::{CoreSession, SessionConfig};
use matchbox_socket::{
    ChannelConfig, PeerState, RtcIceServerConfig, WebRtcSocket, WebRtcSocketBuilder,
};

use super::loopback_peer_session::{play_session, PeerRun};

const BLOB_LEN: usize = 16 * 1024;
const GGRS_CHANNEL: usize = 0;
const RELIABLE_CHANNEL: usize = 1;

/// Runs the peer when argv[1] is `--netplay-spike-peer`; `None` otherwise.
pub(crate) fn run_from_args() -> Option<ExitCode> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--netplay-spike-peer") {
        return None;
    }
    Some(match run(&args[2..]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("LOOPBACK error: {error:#}");
            ExitCode::from(1)
        }
    })
}

fn run(args: &[String]) -> anyhow::Result<()> {
    let [url, core_id, core_path, rom, data_dir, rest @ ..] = args else {
        bail!("usage: --netplay-spike-peer <room-url> <core-id> <core-path> <rom> <data-dir> [frames]");
    };
    let frames: i32 = rest.first().map_or(Ok(600), |f| f.parse())?;
    let (mut socket, message_loop) = WebRtcSocketBuilder::new(url.as_str())
        .ice_server(ice_server())
        .add_channel(ChannelConfig::unreliable())
        .add_channel(ChannelConfig::reliable())
        .build();
    // `MessageLoopFuture` is `Send` on native: it runs on its own thread, the socket
    // stays on the core thread.
    std::thread::spawn(move || futures::executor::block_on(message_loop));
    let peer = wait_for_peer(&mut socket)?;
    let players = socket.players();
    let local = players
        .iter()
        .position(|p| matches!(p, ggrs::PlayerType::Local))
        .context("no local player")?;
    eprintln!("LOOPBACK handles={} local_handle={local}", players.len());
    swap_blob(&mut socket, peer, local)?;
    let mut session = load_core(
        core_id,
        Path::new(core_path),
        Path::new(rom),
        Path::new(data_dir),
    )?;
    let boot = session.serialize().context("boot state")?;
    let channel = std::rc::Rc::new(std::cell::RefCell::new(socket.take_channel(GGRS_CHANNEL)?));
    let mut run = PeerRun {
        socket: &mut socket,
        players: &players,
        local,
        frames,
        channel,
    };
    for epoch in [1u8, 2] {
        session.unserialize(&boot).context("back to boot state")?;
        play_session(&mut run, &mut session, epoch)?;
    }
    observe_exit(&mut socket, peer, local);
    Ok(())
}

/// `NOSTYRA_SPIKE_ICE_URLS` (comma list, e.g. a TURN-only list for the relay probe;
/// `NOSTYRA_SPIKE_ICE_USER` / `_CREDENTIAL`), else matchbox's default STUN.
fn ice_server() -> RtcIceServerConfig {
    let Ok(urls) = std::env::var("NOSTYRA_SPIKE_ICE_URLS") else {
        return RtcIceServerConfig::default();
    };
    RtcIceServerConfig {
        urls: urls.split(',').map(str::to_owned).collect(),
        username: std::env::var("NOSTYRA_SPIKE_ICE_USER").ok(),
        credential: std::env::var("NOSTYRA_SPIKE_ICE_CREDENTIAL").ok(),
    }
}

fn wait_for_peer(socket: &mut WebRtcSocket) -> anyhow::Result<matchbox_socket::PeerId> {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        for (peer, state) in socket.update_peers() {
            if state == PeerState::Connected {
                return Ok(peer);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    bail!("no peer connected within 60 s")
}

/// Handle 0 sends a 16 KiB blob on the reliable channel; handle 1 checks it arrived whole.
fn swap_blob(
    socket: &mut WebRtcSocket,
    peer: matchbox_socket::PeerId,
    local: usize,
) -> anyhow::Result<()> {
    let blob: Vec<u8> = (0..BLOB_LEN).map(|i| (i * 7 % 251) as u8).collect();
    let started = Instant::now();
    if local == 0 {
        socket
            .channel_mut(RELIABLE_CHANNEL)
            .send(blob.clone().into(), peer);
    }
    let mut got = Vec::new();
    while local == 1 && got.len() < BLOB_LEN && started.elapsed() < Duration::from_secs(10) {
        for (_, packet) in socket.channel_mut(RELIABLE_CHANNEL).receive() {
            got.extend_from_slice(&packet);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if local == 1 {
        anyhow::ensure!(
            got == blob,
            "reliable blob: {} of {BLOB_LEN} bytes",
            got.len()
        );
        eprintln!(
            "LOOPBACK reliable_blob=ok bytes={BLOB_LEN} ms={}",
            started.elapsed().as_millis()
        );
    }
    Ok(())
}

fn load_core(core_id: &str, core: &Path, rom: &Path, data: &Path) -> anyhow::Result<CoreSession> {
    let spec = system_catalog::core(core_id).context("unknown core id")?;
    let options: Vec<(String, String)> = spec
        .fixed_options
        .iter()
        .map(|&(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    std::fs::create_dir_all(data)?;
    let mut session = CoreSession::load(SessionConfig {
        core_path: core,
        rom_path: rom,
        system_dir: data,
        save_dir: data,
        rom_data: None,
        staging_dir: None,
        core_options: &options,
        hw_render: None,
        keep_library_loaded: spec.keep_library_loaded,
        stage_content: spec.stage_content,
    })?;
    session.set_controller_port_devices(spec.port_devices);
    Ok(session)
}

/// Handle 0 leaves; handle 1 reports when it sees the peer go.
fn observe_exit(socket: &mut WebRtcSocket, peer: matchbox_socket::PeerId, local: usize) {
    if local == 0 {
        return;
    }
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        let gone = socket.try_update_peers().map_or(true, |changes| {
            changes.contains(&(peer, PeerState::Disconnected))
        });
        if gone {
            eprintln!(
                "LOOPBACK peer_left_seen_after_ms={}",
                started.elapsed().as_millis()
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!("LOOPBACK peer_left_seen=no (10 s)");
}
