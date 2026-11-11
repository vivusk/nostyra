//! Fake room API + shared helpers of the host / join flow tests.

use std::sync::Mutex;

use config_layers::NetplayConfig;
use netplay_session::room_api_types::{
    CreateRoomResponse, IceServer, JoinResponse, RoomRosterEntry,
};

use super::*;

pub const TOKEN: &str = "seat.tok3n";
pub const CREDENTIAL: &str = "TURNCRED42";

#[derive(Default)]
pub struct FakeRoomApi {
    pub created: Mutex<Vec<CreateRoomRequest>>,
    pub joined: Mutex<Vec<(String, JoinRequest)>>,
    pub started: Mutex<Vec<(String, String)>>,
    pub join_error: Option<NetplayError>,
}

pub fn ice() -> Vec<IceServer> {
    vec![IceServer {
        urls: vec!["turn:turn.example:3478?transport=udp".into()],
        username: Some("1700:peer".into()),
        credential: Some(CREDENTIAL.into()),
    }]
}

pub fn hint() -> GameHint {
    GameHint {
        content_sha1: "ab".repeat(20),
        title: "Super Game".into(),
        system: "snes".into(),
        core_id: "snes9x".into(),
        core_version: "1.62".into(),
    }
}

impl RoomApi for FakeRoomApi {
    fn create(&self, body: &CreateRoomRequest) -> Result<CreateRoomResponse, NetplayError> {
        self.created.lock().unwrap().push(body.clone());
        Ok(CreateRoomResponse {
            code: "K7Q2ZP".into(),
            room_id: "room1".into(),
            token: TOKEN.into(),
            ice_servers: ice(),
            expires_at: 99,
        })
    }
    fn join(&self, code: &str, body: &JoinRequest) -> Result<JoinResponse, NetplayError> {
        self.joined
            .lock()
            .unwrap()
            .push((code.into(), body.clone()));
        if let Some(err) = &self.join_error {
            return Err(err.clone());
        }
        Ok(JoinResponse {
            room_id: "room1".into(),
            token: TOKEN.into(),
            ice_servers: ice(),
            roster: vec![RoomRosterEntry {
                nickname: "Ann".into(),
                role: SeatRole::Host,
                connected: true,
            }],
            game: hint(),
            max_players: 4,
            expires_at: 99,
        })
    }
    fn start(&self, code: &str, host_token: &str) -> Result<(), NetplayError> {
        self.started
            .lock()
            .unwrap()
            .push((code.into(), host_token.into()));
        Ok(())
    }
}

pub fn settings() -> NetplaySettings {
    NetplaySettings::from_config(&NetplayConfig::default(), Some("Vivi"), None)
}

pub fn host(api: &Arc<FakeRoomApi>) -> (String, NetplayPlay) {
    let room = HostRoom {
        game: hint(),
        players: 2,
        allow_spectators: true,
    };
    open_host_room(api.clone(), false, &settings(), room).unwrap()
}

pub fn args_text(play: &NetplayPlay) -> String {
    play.args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn session_env(play: &NetplayPlay) -> NetplaySessionEnv {
    NetplaySessionEnv::from_json(&play.env().1.to_string_lossy()).unwrap()
}
