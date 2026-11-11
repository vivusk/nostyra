//! Host / join over REST, then what the runner gets (M20): `--netplay-*` flags and
//! the `NOSTYRA_NETPLAY_SESSION` env JSON (ws url, room id, token, ICE servers with
//! the TURN credential, player cap; the code for the host only). No fingerprint is
//! built here: the runner computes it after loading the game (F4).

use std::ffi::OsString;
use std::sync::Arc;

use ipc_protocol::NetplayEvent;
use netplay_session::netplay_session_env::{NetplaySessionEnv, ENV_KEY};
use netplay_session::room_api_rules::clean_room_code;
use netplay_session::room_api_types::{CreateRoomRequest, GameHint, JoinRequest, SeatRole};
use netplay_session::NetplayError;
use runner_session::SessionUpdate;

use crate::command_error::{CommandError, CommandResult};
use crate::commands::netplay_room_api::{netplay_error, RoomApi, NETPLAY_ERROR_KIND};
use crate::commands::netplay_settings::NetplaySettings;

/// Runner flags + env of one online session, and what the session worker keeps.
pub struct NetplayPlay {
    pub role: SeatRole,
    pub args: Vec<OsString>,
    /// `NOSTYRA_NETPLAY_SESSION` value: holds the token and TURN credential.
    env_json: String,
    /// Host only: marks the room started on the runner's `Started` (F22).
    pub start: Option<HostStartMarker>,
}

impl NetplayPlay {
    /// `(NOSTYRA_NETPLAY_SESSION, json)` for `RunnerLaunch.env` (redacted in Debug).
    pub fn env(&self) -> (OsString, OsString) {
        (ENV_KEY.into(), self.env_json.clone().into())
    }
}

/// The host's room seat: `POST /start` once the runner reports ggrs frame 0.
pub struct HostStartMarker {
    api: Arc<dyn RoomApi>,
    code: String,
    token: String,
    sent: bool,
}

impl HostStartMarker {
    /// Called for every session update (session worker thread, blocking call).
    /// Failure is only logged: the lobby already refuses late joiners.
    pub fn on_update(&mut self, update: &SessionUpdate) {
        if self.sent || !matches!(update, SessionUpdate::Netplay(NetplayEvent::Started { .. })) {
            return;
        }
        self.sent = true;
        if let Err(err) = self.api.start(&self.code, &self.token) {
            tracing::warn!(%err, "netplay: cannot mark the room started");
        }
    }
}

/// Host room parameters.
pub struct HostRoom {
    pub game: GameHint,
    pub players: u8,
    pub allow_spectators: bool,
}

/// `POST /v1/rooms`, then the host's runner parts. Returns the room code (shown in
/// the lobby dialog) with them.
pub fn open_host_room(
    api: Arc<dyn RoomApi>,
    offline: bool,
    settings: &NetplaySettings,
    room: HostRoom,
) -> CommandResult<(String, NetplayPlay)> {
    if offline {
        return Err(netplay_error(&NetplayError::Offline));
    }
    let urls = settings.urls()?;
    let reply = api
        .create(&CreateRoomRequest {
            nickname: settings.nickname.clone(),
            game: room.game,
            max_players: room.players,
            allow_spectators: room.allow_spectators,
        })
        .map_err(|err| netplay_error(&err))?;
    let env = NetplaySessionEnv {
        server_ws_url: urls.ws_base,
        room_id: reply.room_id,
        code: Some(reply.code.clone()),
        token: reply.token.clone(),
        role: SeatRole::Host,
        max_players: room.players,
        ice_servers: reply.ice_servers,
        expires_at: reply.expires_at,
    };
    let start = HostStartMarker {
        api,
        code: reply.code.clone(),
        token: reply.token,
        sent: false,
    };
    let play = runner_parts(settings, env, Some(start))?;
    Ok((reply.code, play))
}

/// `POST /v1/rooms/{code}/join`, the local copy of the host's game (`find_game` by
/// content sha1), then the guest's runner parts (`max_players` from the reply, F21).
pub fn open_join_seat(
    api: &dyn RoomApi,
    offline: bool,
    settings: &NetplaySettings,
    code: &str,
    role: SeatRole,
    find_game: impl FnOnce(&str) -> CommandResult<Option<i64>>,
) -> CommandResult<(i64, NetplayPlay)> {
    if role == SeatRole::Host {
        return Err(CommandError::invalid("Join as a player or a spectator."));
    }
    let code = clean_room_code(code).map_err(|err| netplay_error(&err))?;
    if offline {
        return Err(netplay_error(&NetplayError::Offline));
    }
    let urls = settings.urls()?;
    let body = JoinRequest {
        nickname: settings.nickname.clone(),
        role,
    };
    let reply = api.join(&code, &body).map_err(|err| netplay_error(&err))?;
    let game_id = find_game(&reply.game.content_sha1)?.ok_or_else(|| missing_game(&reply.game))?;
    let env = NetplaySessionEnv {
        server_ws_url: urls.ws_base,
        room_id: reply.room_id,
        code: None,
        token: reply.token,
        role,
        max_players: reply.max_players,
        ice_servers: reply.ice_servers,
        expires_at: reply.expires_at,
    };
    Ok((game_id, runner_parts(settings, env, None)?))
}

/// "You need <title> (<system>) in your library".
fn missing_game(game: &GameHint) -> CommandError {
    let system =
        system_catalog::system(&game.system).map_or(game.system.as_str(), |s| s.short_label);
    CommandError::new(
        NETPLAY_ERROR_KIND,
        format!("You need {} ({system}) in your library", game.title),
    )
}

fn runner_parts(
    settings: &NetplaySettings,
    env: NetplaySessionEnv,
    start: Option<HostStartMarker>,
) -> CommandResult<NetplayPlay> {
    env.validate().map_err(|err| netplay_error(&err))?;
    let env_json = env.to_json().map_err(|err| netplay_error(&err))?;
    Ok(NetplayPlay {
        role: env.role,
        args: settings.runner_args(env.role, env.max_players),
        env_json,
        start,
    })
}

impl std::fmt::Debug for NetplayPlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetplayPlay")
            .field("role", &self.role)
            .field("args", &self.args)
            .field("env_json", &"<redacted>")
            .field("start", &self.start.as_ref().map(|_| "<host>"))
            .finish()
    }
}

#[cfg(test)]
#[path = "netplay_room_flow_fake_api.rs"]
mod fake_api;
#[cfg(test)]
#[path = "netplay_room_flow_join_tests.rs"]
mod join_tests;
#[cfg(test)]
#[path = "netplay_room_flow_tests.rs"]
mod tests;
