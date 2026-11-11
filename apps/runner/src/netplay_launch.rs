//! Netplay around the core load (M20 phase 04). Before the load: the session env
//! (`NOSTYRA_NETPLAY_SESSION`, taken out of the process env at start), the CLI
//! role / player checks and the registry eligibility; the room's `max_players`
//! then picks the multitap options and port devices for every role (F21). After
//! the load: the authoritative eligibility with the core's quirks, the
//! `MUST_INITIALIZE` warm-up, the fingerprint and the socket — all before the
//! window opens. Refusals emit `Netplay{Ended}` and exit with their code.

use input_mapping::{NetplayInputSource, PortState};
use ipc_protocol::{NetplayEndReason, NetplayEvent, RunnerEvent};
use netplay_session::netplay_session_env::{NetplaySessionEnv, ENV_KEY};
use netplay_session::room_api_types::SeatRole;
use system_catalog::{NetplaySupport, SystemSpec};

use crate::netplay_gate::NetplaySeat;
use crate::netplay_runtime::netplay_exit::NetplayExit;
use crate::netplay_runtime::NetplayRuntime;
use crate::runner_cli_args::{NetplayArgs, NetplayRoleArg, RunnerCliArgs};
use crate::runner_core_options::LOCAL_PLAYERS;
use crate::session_reporter::SessionReporter;

/// The session env JSON, removed from the process env (call before any thread).
pub fn take_session_env() -> Option<String> {
    let value = std::env::var(ENV_KEY).ok();
    std::env::remove_var(ENV_KEY);
    value
}

/// A checked netplay launch, before the core loads.
pub struct NetplayLaunch {
    env: NetplaySessionEnv,
    args: NetplayArgs,
    support: NetplaySupport,
}

mod session_start;

pub use session_start::{start, LoadedFacts};

/// Everything the window loop takes over.
pub struct NetplayAttach {
    pub runtime: NetplayRuntime,
    pub seat: NetplaySeat,
    /// `--netplay-scripted-input` port (test flag).
    pub scripted: Option<PortState>,
    pub hud_on: bool,
    pub window_hidden: bool,
    pub input: NetplayInputSource,
    pub analog: bool,
}

/// Room players resolved before the load (local play: 2, the usual devices).
pub fn players(launch: Option<&NetplayLaunch>) -> u8 {
    launch.map_or(LOCAL_PLAYERS, |launch| launch.env.max_players)
}

fn refuse(
    reporter: &SessionReporter,
    exit: NetplayExit,
    reason: NetplayEndReason,
) -> anyhow::Error {
    reporter.emit(&RunnerEvent::Netplay(NetplayEvent::Ended { reason }));
    exit.into()
}

fn error_reason(message: &str) -> NetplayEndReason {
    NetplayEndReason::Error {
        message: message.to_owned(),
    }
}

/// Checks the CLI, the env and the registry; `None` outside netplay.
pub fn prepare(
    args: &RunnerCliArgs,
    system: &SystemSpec,
    env_json: Option<String>,
    reporter: &SessionReporter,
) -> anyhow::Result<Option<NetplayLaunch>> {
    let Some(role) = args.netplay.role else {
        return Ok(None);
    };
    let usage = |text: &str| refuse(reporter, NetplayExit::usage(text), error_reason(text));
    if args.headless_frames.is_some() || args.print_config {
        return Err(usage("--netplay needs the game window"));
    }
    let env = env_json
        .ok_or_else(|| usage("NOSTYRA_NETPLAY_SESSION is not set"))
        .and_then(|json| NetplaySessionEnv::from_json(&json).map_err(|e| usage(&e.to_string())))?;
    let expected = match role {
        NetplayRoleArg::Host => SeatRole::Host,
        NetplayRoleArg::Join => SeatRole::Player,
        NetplayRoleArg::Watch => SeatRole::Spectator,
    };
    if env.role != expected {
        return Err(usage("--netplay role differs from the room seat"));
    }
    if role == NetplayRoleArg::Host && args.netplay.players != Some(env.max_players) {
        return Err(usage(
            "--netplay-players differs from the room's player cap",
        ));
    }
    let core = system_catalog::core_for_system(system.id);
    let support = core
        .ok_or("no core for this system")
        .and_then(|core| {
            system_catalog::netplay_eligibility(core, core.savestates, None)
                .map_err(|_| "this core cannot play online")
        })
        .map_err(|text| {
            let reason = NetplayEndReason::Ineligible {
                reason: text.to_owned(),
            };
            refuse(reporter, NetplayExit::ineligible(text), reason)
        })?;
    let args = args.netplay.clone();
    Ok(Some(NetplayLaunch { env, args, support }))
}

/// Spectators get no achievement session (they never play).
pub fn is_spectator(launch: Option<&NetplayLaunch>) -> bool {
    launch.is_some_and(|launch| launch.env.role == SeatRole::Spectator)
}

#[cfg(test)]
#[path = "netplay_launch_tests.rs"]
mod tests;
