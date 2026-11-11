//! After the core load: the authoritative eligibility (the core's quirks), the
//! `MUST_INITIALIZE` warm-up, the fingerprint and the room socket, before the
//! window opens (split from `netplay_launch.rs`).

use std::path::Path;
use std::time::Instant;

use input_mapping::NetplayInputSource;
use ipc_protocol::NetplayEndReason;
use libretro_host::{CoreSession, NullVideoSink};
use netplay_session::room_api_types::SeatRole;
use netplay_session::socket_thread::NetplaySocket;
use system_catalog::{NetplayQuirks, NetplaySupport};

use super::{error_reason, refuse, NetplayAttach, NetplayLaunch};
use crate::netplay_gate::NetplaySeat;
use crate::netplay_runtime::fingerprint_source::{build_fingerprint, FingerprintInputs};
use crate::netplay_runtime::netplay_core::{run_warmup, NetplayCore};
use crate::netplay_runtime::netplay_exit::NetplayExit;
use crate::netplay_runtime::session_core::{SessionCore, SilentHooks};
use crate::netplay_runtime::{NetplayRuntime, RuntimeOptions};
use crate::rom_soft_patching::SoftPatchedRom;
use crate::runner_data_paths::RunnerDataPaths;
use crate::session_reporter::SessionReporter;

/// What the loaded session's fingerprint is built from.
pub struct LoadedFacts<'a> {
    pub core_path: &'a Path,
    pub content_rom: &'a Path,
    pub patched: &'a SoftPatchedRom,
    pub paths: &'a RunnerDataPaths,
    pub core_options: &'a [(String, String)],
}

/// After the load: eligibility with the core's quirks, warm-up, fingerprint, socket.
pub fn start(
    launch: NetplayLaunch,
    session: &mut CoreSession,
    facts: &LoadedFacts<'_>,
    reporter: &SessionReporter,
) -> anyhow::Result<NetplayAttach> {
    let system = facts.paths.system;
    let ineligible = |text: &str| {
        let reason = NetplayEndReason::Ineligible {
            reason: text.to_owned(),
        };
        refuse(reporter, NetplayExit::ineligible(text), reason)
    };
    let core = system_catalog::core_for_system(system.id).ok_or_else(|| ineligible("no core"))?;
    let quirks = NetplayQuirks {
        incomplete: session.serialization_quirks().incomplete(),
    };
    system_catalog::netplay_eligibility(core, core.savestates, Some(quirks))
        .map_err(|_| ineligible("this core's save states cannot drive rollback"))?;
    if launch.support == NetplaySupport::Experimental {
        tracing::warn!(core = core.id, "netplay on an Experimental core");
    }
    let analog = system.analog_sticks != system_catalog::AnalogSticks::None;
    let mut input = NetplayInputSource::default();
    let (mut video, mut hooks) = (NullVideoSink::default(), SilentHooks::default());
    let mut core_view = SessionCore {
        session,
        video: &mut video,
        input: &mut input,
        analog,
        hooks: &mut hooks,
    };
    let warmup_frames =
        run_warmup(&mut core_view).ok_or_else(|| ineligible("the core never became ready"))?;
    let (sram_len, rtc_len) = core_view.memory_sizes();
    let started = Instant::now();
    let fingerprint = build_fingerprint(&FingerprintInputs {
        core,
        system,
        core_path: facts.core_path,
        core_version: &session.core_info().version,
        content_sha1: &facts.paths.content_sha1,
        content_rom: facts.content_rom,
        patched: facts.patched,
        system_dir: &facts.paths.system_dir,
        core_options: facts.core_options,
        fps: session.av_info().timing.fps,
        portable_states: session.serialization_quirks().portable(),
        warmup_frames,
        max_players: launch.env.max_players,
    })?;
    tracing::debug!(
        ms = started.elapsed().as_millis(),
        warmup_frames,
        "netplay fingerprint"
    );
    let socket = NetplaySocket::open(&launch.env, launch.args.relay_only).map_err(|err| {
        refuse(
            reporter,
            NetplayExit::usage(err.to_string()),
            error_reason(&err.to_string()),
        )
    })?;
    let nickname = launch
        .args
        .nickname
        .clone()
        .or_else(|| std::env::var("USERNAME").ok());
    let options = RuntimeOptions {
        role: launch.env.role,
        nickname: nickname.unwrap_or_else(|| "Player".to_owned()),
        fingerprint,
        sram_len,
        rtc_len,
        input_delay: launch.args.input_delay.and_then(|d| d.frames()),
        autostart: launch.args.autostart,
        frames_limit: launch.args.frames_limit,
        experimental: launch.support == NetplaySupport::Experimental,
        room_code: launch.env.code.clone(),
    };
    let seat = match launch.env.role {
        SeatRole::Host => NetplaySeat::Host,
        SeatRole::Player => NetplaySeat::Player,
        SeatRole::Spectator => NetplaySeat::Spectator,
    };
    Ok(NetplayAttach {
        runtime: NetplayRuntime::new(Box::new(socket), options, Instant::now()),
        seat,
        scripted: launch.args.scripted_input.map(|held| held.port_state()),
        hud_on: launch.args.hud_on(),
        window_hidden: launch.args.window_hidden,
        input,
        analog,
    })
}
