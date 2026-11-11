//! Registry-only netplay eligibility (F12: the runner repeats it with the loaded
//! core's quirks) and what the Detail page shows about Play online.

use netplay_session::room_api_client::offline_from_env;
use serde::Serialize;
use system_catalog::{netplay_eligibility, netplay_max_players, CoreSpec, NetplaySupport};

use crate::command_error::{CommandError, CommandResult};
use crate::commands::netplay_room_api::NETPLAY_ERROR_KIND;

/// Registry-only eligibility of `spec` (no loaded-core quirks yet).
pub fn registry_precheck(spec: &CoreSpec, platform_id: &str) -> CommandResult<NetplaySupport> {
    netplay_eligibility(spec, spec.savestates, None).map_err(|_| {
        let system = system_catalog::system(platform_id).map_or(platform_id, |s| s.short_label);
        CommandError::new(
            NETPLAY_ERROR_KIND,
            format!("{system} games cannot be played online."),
        )
    })
}

/// Largest room the host may open (the server takes 2..=4; one-pad systems: 2).
pub fn room_player_cap(spec: &CoreSpec, platform_id: &str) -> u8 {
    netplay_max_players(spec, platform_id).clamp(2, 4)
}

/// Play online facts of a game for the Detail page (registry only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetplayGameView {
    /// `"yes"`, `"experimental"` (badge + warning) or `"no"`.
    pub support: &'static str,
    /// Largest room the host may open (2..=4).
    pub max_players: u8,
    /// `NOSTYRA_OFFLINE=1`: the menu shows disabled with a reason.
    pub offline: bool,
}

/// [`NetplayGameView`] of a game of `platform_id`.
pub fn netplay_support_of(platform_id: &str) -> NetplayGameView {
    let offline = offline_from_env();
    let Some(spec) = system_catalog::core_for_system(platform_id) else {
        return NetplayGameView {
            support: "no",
            max_players: 2,
            offline,
        };
    };
    let support = match registry_precheck(spec, platform_id) {
        Ok(NetplaySupport::Yes) => "yes",
        Ok(NetplaySupport::Experimental) => "experimental",
        _ => "no",
    };
    NetplayGameView {
        support,
        max_players: room_player_cap(spec, platform_id),
        offline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_precheck_and_dto_labels() {
        let snes = system_catalog::core_for_system("snes").unwrap();
        assert_eq!(
            registry_precheck(snes, "snes").unwrap(),
            NetplaySupport::Yes
        );
        let nds = system_catalog::core_for_system("nds").unwrap();
        let err = registry_precheck(nds, "nds").unwrap_err();
        assert_eq!(err.kind, NETPLAY_ERROR_KIND);
        assert!(
            err.message.contains("cannot be played online"),
            "{}",
            err.message
        );
        let view = |id: &str| {
            let view = netplay_support_of(id);
            (view.support, view.max_players)
        };
        assert_eq!(view("nds").0, "no");
        assert_eq!(view("snes"), ("yes", 4));
        assert_eq!(view("psx").0, "experimental");
        assert_eq!(view("gba"), ("yes", 2), "one pad: smallest room");
        assert_eq!(view("no-such-system"), ("no", 2));
    }
}
