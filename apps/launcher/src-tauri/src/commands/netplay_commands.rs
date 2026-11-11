//! Play online (M20): host a room, join one by code as a player or spectator, poll
//! a room's status. The launcher only talks REST; the runner owns the network once
//! spawned (`start_game` with the netplay flags + env). Registry-only eligibility
//! here (F12): the runner repeats it with the loaded core's quirks.

use std::sync::Arc;

use ipc_protocol::StateSlotId;
use netplay_session::room_api_client::{offline_from_env, RoomApiClient};
use netplay_session::room_api_types::{GameHint, SeatRole};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::app_state::{lock, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;
use crate::commands::core_resolution::{core_for_platform, installed_core_for_platform};
use crate::commands::netplay_eligibility_view::{registry_precheck, room_player_cap};
use crate::commands::netplay_room_api::netplay_error;
use crate::commands::netplay_room_flow::{open_host_room, open_join_seat, HostRoom};
use crate::commands::netplay_settings::NetplaySettings;
use crate::commands::play_commands::start_game;

/// `netplay_host` reply: the room code for the lobby dialog (never the token).
#[derive(Debug, Clone, Serialize)]
pub struct NetplayHostReply {
    pub code: String,
}

/// Seat a joiner asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JoinRole {
    Player,
    Spectator,
}

/// `GET /v1/rooms/{code}` for the lobby dialog's poll.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomStatusView {
    pub players: usize,
    pub spectators: usize,
    pub started: bool,
    pub max_players: u8,
}

/// Hosts `game_id` in a new room of `players` (2..=the system's cap), then starts
/// the runner (cloud pull / resume choice already done by the UI: `load_state`).
#[tauri::command]
pub async fn netplay_host(
    app: AppHandle,
    game_id: i64,
    players: u8,
    allow_spectators: bool,
    load_state: Option<StateSlotId>,
) -> CommandResult<NetplayHostReply> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let game = storage_db::get_game(&state.db(), game_id)?
            .ok_or_else(|| CommandError::invalid("This game is no longer in the library."))?;
        if game.missing {
            return Err(CommandError::invalid("The ROM file was not found."));
        }
        let (spec, _) = installed_core_for_platform(&state.paths.cores_dir, &game.platform_id)?;
        registry_precheck(spec, &game.platform_id)?;
        if !(2..=room_player_cap(spec, &game.platform_id)).contains(&players) {
            return Err(CommandError::invalid("Unsupported number of players."));
        }
        ensure_no_session(&state)?;
        let core_version = storage_db::get_installed_core(&state.db(), spec.id)?
            .and_then(|core| core.version)
            .unwrap_or_default();
        let settings = NetplaySettings::load(&state.paths.data_root);
        let room = HostRoom {
            game: GameHint {
                content_sha1: game.sha1,
                title: game.title,
                system: game.platform_id,
                core_id: spec.id.to_string(),
                core_version,
            },
            players,
            allow_spectators,
        };
        let api = Arc::new(live_client(&state, &settings)?);
        let (code, play) = open_host_room(api, offline_from_env(), &settings, room)?;
        start_game(&app, game_id, load_state, Some(play))?;
        Ok(NetplayHostReply { code })
    })
    .await
}

/// Joins room `code`: finds the host's game in the library by content sha1 and
/// starts the runner as a player / spectator (no cloud pull, no resume: guests
/// write nothing). Returns the local game id.
#[tauri::command]
pub async fn netplay_join(app: AppHandle, code: String, role: JoinRole) -> CommandResult<i64> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        ensure_no_session(&state)?;
        let settings = NetplaySettings::load(&state.paths.data_root);
        let api = live_client(&state, &settings)?;
        let seat = match role {
            JoinRole::Player => SeatRole::Player,
            JoinRole::Spectator => SeatRole::Spectator,
        };
        let find = |sha1: &str| Ok(storage_db::find_game_id_by_sha1(&state.db(), sha1)?);
        let (game_id, play) =
            open_join_seat(&api, offline_from_env(), &settings, &code, seat, find)?;
        let platform = storage_db::get_game(&state.db(), game_id)?
            .map(|game| game.platform_id)
            .ok_or_else(|| CommandError::invalid("This game is no longer in the library."))?;
        registry_precheck(core_for_platform(&platform)?, &platform)?;
        start_game(&app, game_id, None, Some(play))?;
        Ok(game_id)
    })
    .await
}

/// Room counts for the lobby dialog before the runner reports its own roster.
#[tauri::command]
pub async fn netplay_room_status(app: AppHandle, code: String) -> CommandResult<RoomStatusView> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let settings = NetplaySettings::load(&state.paths.data_root);
        let status = live_client(&state, &settings)?
            .status(&code)
            .map_err(|err| netplay_error(&err))?;
        Ok(RoomStatusView {
            players: status.players,
            spectators: status.spectators,
            started: status.started,
            max_players: status.max_players,
        })
    })
    .await
}

/// README Privacy section (who sees your IP address in an online room).
const PRIVACY_URL: &str = "https://github.com/vivusk/nostyra#privacy";

/// Settings → Netplay "Privacy" link (fixed url, never from the UI).
#[tauri::command]
pub async fn open_netplay_privacy() -> CommandResult<()> {
    run_blocking(|| {
        tauri_plugin_opener::open_url(PRIVACY_URL, None::<&str>)
            .map_err(|err| CommandError::new("storage", format!("Cannot open the browser: {err}")))
    })
    .await
}

/// Refused early (before any REST call): one session per launcher.
fn ensure_no_session(state: &AppState) -> CommandResult<()> {
    match lock(&state.session).is_some() {
        true => Err(CommandError::new("session", "A game is already running.")),
        false => Ok(()),
    }
}

/// The room API on the shared HTTPS client (`NOSTYRA_OFFLINE` honoured).
fn live_client(state: &AppState, settings: &NetplaySettings) -> CommandResult<RoomApiClient> {
    let urls = settings.urls()?;
    Ok(RoomApiClient::new(
        &urls,
        Box::new(state.http.clone()),
        offline_from_env(),
    ))
}
