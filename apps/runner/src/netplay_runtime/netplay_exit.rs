//! How a netplay session ends the runner: exit code (ipc-protocol) + the launcher's
//! `Netplay{Ended}` reason. Raised as an error so `main` maps it to the code like
//! any other failure (`runner_exit_codes::exit_code_for`).

use std::fmt;

use ipc_protocol::{NetplayEndReason, EXIT_NETPLAY_HOST_LEFT, EXIT_NETPLAY_INCOMPATIBLE};
use ipc_protocol::{EXIT_NETPLAY_INELIGIBLE, EXIT_RUNTIME, EXIT_USAGE};
use netplay_session::lobby_state::EndReason;

/// The end of a netplay session with a non-zero exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetplayExit {
    pub code: u8,
    pub message: String,
}

impl NetplayExit {
    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(EXIT_USAGE, message)
    }

    pub fn ineligible(message: impl Into<String>) -> Self {
        Self::new(EXIT_NETPLAY_INELIGIBLE, message)
    }

    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code: code as u8,
            message: message.into(),
        }
    }
}

impl fmt::Display for NetplayExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for NetplayExit {}

/// Launcher reason + exit (`None` = clean exit 0) of a lobby end.
pub(crate) fn end_outcome(reason: &EndReason) -> (NetplayEndReason, Option<NetplayExit>) {
    match reason {
        EndReason::HostLeft => (
            NetplayEndReason::HostLeft,
            Some(NetplayExit::new(
                EXIT_NETPLAY_HOST_LEFT,
                "The host left the game",
            )),
        ),
        EndReason::Refused(refusal) => {
            let text = refusal.text();
            let mismatches: Vec<String> = text.split("; ").map(str::to_owned).collect();
            let exit = NetplayExit::new(EXIT_NETPLAY_INCOMPATIBLE, text);
            (NetplayEndReason::Refused { mismatches }, Some(exit))
        }
        EndReason::Left => (NetplayEndReason::Quit, None),
        EndReason::Error(message) => (
            NetplayEndReason::Error {
                message: message.clone(),
            },
            Some(NetplayExit::new(
                EXIT_RUNTIME,
                format!("netplay: {message}"),
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use netplay_session::lobby_protocol::RefusalReason;

    use super::*;

    #[test]
    fn end_reasons_map_to_exit_codes() {
        let (reason, exit) = end_outcome(&EndReason::HostLeft);
        assert_eq!(reason, NetplayEndReason::HostLeft);
        assert_eq!(
            exit.map(|e| i32::from(e.code)),
            Some(EXIT_NETPLAY_HOST_LEFT)
        );
        let (_, exit) = end_outcome(&EndReason::Refused(RefusalReason::RoomFull));
        assert_eq!(
            exit.map(|e| i32::from(e.code)),
            Some(EXIT_NETPLAY_INCOMPATIBLE)
        );
        assert_eq!(
            end_outcome(&EndReason::Left),
            (NetplayEndReason::Quit, None)
        );
        let (_, exit) = end_outcome(&EndReason::Error("x".into()));
        assert_eq!(exit.map(|e| i32::from(e.code)), Some(EXIT_RUNTIME));
        assert_eq!(
            i32::from(NetplayExit::ineligible("no").code),
            EXIT_NETPLAY_INELIGIBLE
        );
        assert_eq!(i32::from(NetplayExit::usage("bad").code), EXIT_USAGE);
    }
}
