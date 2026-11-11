//! The signaling server's room API as the launcher sees it (M20): a small trait over
//! `netplay_session::room_api_client::RoomApiClient` so the host / join flows and
//! the `/start` marking are testable with a fake, and the mapping of
//! [`NetplayError`]s to the dialog texts ("Game already started", "Room is full").

use netplay_session::room_api_client::RoomApiClient;
use netplay_session::room_api_types::{
    CreateRoomRequest, CreateRoomResponse, JoinRequest, JoinResponse,
};
use netplay_session::NetplayError;

use crate::command_error::CommandError;

/// `/v1/rooms*` calls of the host / join flows (blocking; off the UI thread).
pub trait RoomApi: Send + Sync {
    fn create(&self, body: &CreateRoomRequest) -> Result<CreateRoomResponse, NetplayError>;
    fn join(&self, code: &str, body: &JoinRequest) -> Result<JoinResponse, NetplayError>;
    fn start(&self, code: &str, host_token: &str) -> Result<(), NetplayError>;
}

impl RoomApi for RoomApiClient {
    fn create(&self, body: &CreateRoomRequest) -> Result<CreateRoomResponse, NetplayError> {
        RoomApiClient::create(self, body)
    }
    fn join(&self, code: &str, body: &JoinRequest) -> Result<JoinResponse, NetplayError> {
        RoomApiClient::join(self, code, body)
    }
    fn start(&self, code: &str, host_token: &str) -> Result<(), NetplayError> {
        RoomApiClient::start(self, code, host_token)
    }
}

/// Error kind of every netplay refusal (the dialogs show the message as is).
pub const NETPLAY_ERROR_KIND: &str = "netplay";

/// A readable refusal for the Play online dialogs (never a code, token or url).
pub fn netplay_error(err: &NetplayError) -> CommandError {
    let message = match err {
        NetplayError::Offline => "Online play needs the network (offline mode is on).".into(),
        NetplayError::Network => {
            "Could not reach the netplay server. Check your connection.".into()
        }
        NetplayError::BadServerUrl => "The netplay server address in Settings is not valid.".into(),
        NetplayError::BadRoomCode => "Room codes have 6 letters or digits.".into(),
        NetplayError::RoomNotFound => "Room not found. Check the code.".into(),
        NetplayError::RoomExpired => "This room has expired.".into(),
        NetplayError::RoomFull => "Room is full".into(),
        NetplayError::RoomStarted => "Game already started".into(),
        NetplayError::SpectatorsOff => "This room does not allow spectators.".into(),
        NetplayError::RateLimited { .. } => "Too many requests. Try again in a minute.".into(),
        NetplayError::ServerFull => "The netplay server is full. Try again later.".into(),
        other => format!("Online play failed: {other}"),
    };
    CommandError::new(NETPLAY_ERROR_KIND, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_refusals_read_like_the_dialog() {
        assert_eq!(
            netplay_error(&NetplayError::RoomStarted).message,
            "Game already started"
        );
        assert_eq!(
            netplay_error(&NetplayError::RoomFull).message,
            "Room is full"
        );
        let server = netplay_error(&NetplayError::Server(502));
        assert_eq!(server.kind, NETPLAY_ERROR_KIND);
        assert!(server.message.contains("502"), "{}", server.message);
    }
}
