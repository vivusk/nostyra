//! Settings → Netplay as the launcher uses them (M20): the effective nickname
//! (setting, else the Windows user name, else "Player"), the signaling server (env
//! `NOSTYRA_NETPLAY_SERVER` beats `[netplay] server_url`), and the runner flags
//! they become. The room itself never travels in arguments (env JSON only).

use std::ffi::OsString;
use std::path::Path;

use config_layers::{global_config_path, resolve_layers_with_warnings, NetplayConfig};
use config_layers::{MAX_NICKNAME_CHARS, NETPLAY_DEFAULT_SERVER_URL};
use netplay_session::room_api_rules::ServerUrls;
use netplay_session::room_api_types::SeatRole;

use crate::command_error::{CommandError, CommandResult};

/// Runtime override of the server url (tests, self-hosters).
pub const SERVER_ENV: &str = "NOSTYRA_NETPLAY_SERVER";

/// The global `[netplay]` settings plus the resolved nickname / server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetplaySettings {
    pub nickname: String,
    pub server_url: String,
    pub input_delay: String,
    pub hud: bool,
    pub relay_only: bool,
}

impl NetplaySettings {
    /// From `<data>/config/global.toml` and the process env.
    pub fn load(data_root: &Path) -> Self {
        let global = global_config_path(data_root);
        let (config, _) = resolve_layers_with_warnings(&[global.as_path()], None);
        let user = std::env::var("USERNAME").ok();
        let server = std::env::var(SERVER_ENV).ok();
        Self::from_config(&config.netplay, user.as_deref(), server.as_deref())
    }

    /// `user` = the OS user name, `server_env` = [`SERVER_ENV`] (both optional).
    pub fn from_config(
        config: &NetplayConfig,
        user: Option<&str>,
        server_env: Option<&str>,
    ) -> Self {
        let nickname = [Some(config.nickname.as_str()), user]
            .into_iter()
            .flatten()
            .map(clean_nickname)
            .find(|nick| !nick.is_empty())
            .unwrap_or_else(|| "Player".to_string());
        let server_url = [server_env, Some(config.server_url.as_str())]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|url| !url.is_empty())
            .unwrap_or(NETPLAY_DEFAULT_SERVER_URL)
            .to_string();
        Self {
            nickname,
            server_url,
            input_delay: config.input_delay.clone(),
            hud: config.hud,
            relay_only: config.relay_only,
        }
    }

    /// REST / WebSocket bases of the server.
    pub fn urls(&self) -> CommandResult<ServerUrls> {
        ServerUrls::parse(&self.server_url).map_err(|_| {
            CommandError::new(
                "netplay",
                "The netplay server address in Settings → Netplay is not valid.",
            )
        })
    }

    /// `--netplay <role> [--netplay-players N] --netplay-nickname … …` for the runner.
    pub fn runner_args(&self, role: SeatRole, players: u8) -> Vec<OsString> {
        let role_arg = match role {
            SeatRole::Host => "host",
            SeatRole::Player => "join",
            SeatRole::Spectator => "watch",
        };
        let mut args: Vec<OsString> = vec!["--netplay".into(), role_arg.into()];
        if role == SeatRole::Host {
            args.extend(["--netplay-players".into(), players.to_string().into()]);
        }
        args.extend([
            "--netplay-nickname".into(),
            self.nickname.clone().into(),
            "--netplay-input-delay".into(),
            self.input_delay.clone().into(),
            "--netplay-hud".into(),
            if self.hud { "on" } else { "off" }.into(),
        ]);
        if self.relay_only {
            args.push("--netplay-relay-only".into());
        }
        args
    }
}

/// Trimmed, control characters dropped, at most [`MAX_NICKNAME_CHARS`] characters.
fn clean_nickname(text: &str) -> String {
    text.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NICKNAME_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nickname_and_server_fallbacks() {
        let config = NetplayConfig::default();
        let settings = NetplaySettings::from_config(&config, Some(" duyph "), None);
        assert_eq!(settings.nickname, "duyph");
        assert_eq!(settings.server_url, NETPLAY_DEFAULT_SERVER_URL);
        let none = NetplaySettings::from_config(&config, None, Some(" "));
        assert_eq!(none.nickname, "Player");
        let named = NetplayConfig {
            nickname: "Vivi\u{7}".repeat(20),
            server_url: "wss://own.example".into(),
            ..NetplayConfig::default()
        };
        let set = NetplaySettings::from_config(&named, Some("user"), None);
        assert_eq!(set.nickname.chars().count(), MAX_NICKNAME_CHARS);
        assert!(!set.nickname.contains('\u{7}'));
        assert_eq!(set.server_url, "wss://own.example");
        let env = NetplaySettings::from_config(&named, None, Some("wss://env.example"));
        assert_eq!(env.server_url, "wss://env.example", "env beats the setting");
    }

    #[test]
    fn runner_flags_per_role() {
        let mut settings = NetplaySettings::from_config(&NetplayConfig::default(), None, None);
        let text = |args: Vec<OsString>| {
            args.iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(
            text(settings.runner_args(SeatRole::Host, 2)),
            "--netplay host --netplay-players 2 --netplay-nickname Player \
             --netplay-input-delay auto --netplay-hud on"
        );
        settings.hud = false;
        settings.relay_only = true;
        let watch = text(settings.runner_args(SeatRole::Spectator, 4));
        assert!(
            watch.starts_with("--netplay watch --netplay-nickname"),
            "{watch}"
        );
        assert!(!watch.contains("--netplay-players"), "{watch}");
        assert!(
            watch.ends_with("--netplay-hud off --netplay-relay-only"),
            "{watch}"
        );
        assert!(text(settings.runner_args(SeatRole::Player, 2)).starts_with("--netplay join "));
    }
}
