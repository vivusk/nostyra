//! `[netplay]` section (M20 online play). **Global layer only**, like `[cloud]`: one
//! nickname and one server per PC, so the table is stripped from the system, core
//! and game layers before merging. The launcher reads it and passes the values to
//! the runner as `--netplay-*` flags.

use std::path::Path;

use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::known_key_checks::KeyCheck;

/// Section name of the netplay settings.
pub const NETPLAY_SECTION: &str = "netplay";

/// Signaling server used when `[netplay] server_url` is not set: the build-time
/// `NOSTYRA_NETPLAY_SERVER` value, else the project server.
pub const NETPLAY_DEFAULT_SERVER_URL: &str = match option_env!("NOSTYRA_NETPLAY_SERVER") {
    Some(url) => url,
    None => "wss://netplay.nostyra.com",
};

/// Longest nickname (the server refuses longer ones).
pub const MAX_NICKNAME_CHARS: usize = 32;
/// Largest manual input delay (frames); `"auto"` picks it from the ping.
pub const MAX_NETPLAY_INPUT_DELAY: u8 = 10;

/// Online-play settings (launcher Settings → Netplay, global layer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetplayConfig {
    /// Name shown to the others; empty = the Windows user name (launcher fallback).
    pub nickname: String,
    /// `wss://` / `https://` signaling server (`ws://` / `http://` for local tests).
    pub server_url: String,
    /// `"auto"` or `"0"` ..= `"10"` frames.
    pub input_delay: String,
    /// In-game HUD (ping, rollbacks, relay); warnings show either way.
    pub hud: bool,
    /// Prefer the TURN relay (best effort: direct links may still happen).
    pub relay_only: bool,
}

impl Default for NetplayConfig {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            server_url: NETPLAY_DEFAULT_SERVER_URL.to_string(),
            input_delay: "auto".to_string(),
            hud: true,
            relay_only: false,
        }
    }
}

/// A nickname the server accepts (empty allowed: "use the default").
fn valid_nickname(text: &str) -> bool {
    text.chars().count() <= MAX_NICKNAME_CHARS && !text.chars().any(char::is_control)
}

/// One of the four url schemes, a host and no spaces / query.
pub fn valid_server_url(text: &str) -> bool {
    ["wss://", "https://", "ws://", "http://"]
        .iter()
        .find_map(|scheme| text.strip_prefix(scheme))
        .is_some_and(|rest| {
            !rest.is_empty() && !rest.contains(|c: char| c.is_whitespace() || "?#".contains(c))
        })
}

/// `"auto"` or a frame count within [`MAX_NETPLAY_INPUT_DELAY`].
pub fn valid_input_delay(text: &str) -> bool {
    text == "auto"
        || text
            .parse::<u8>()
            .is_ok_and(|n| n <= MAX_NETPLAY_INPUT_DELAY)
}

/// Value checks of the `[netplay]` keys.
pub(crate) const NETPLAY_KEY_CHECKS: [KeyCheck; 5] = [
    ("netplay", "nickname", |v: &Value| {
        v.as_str().is_some_and(valid_nickname)
    }),
    ("netplay", "server_url", |v: &Value| {
        v.as_str().is_some_and(valid_server_url)
    }),
    ("netplay", "input_delay", |v: &Value| {
        v.as_str().is_some_and(valid_input_delay)
    }),
    ("netplay", "hud", |v: &Value| v.is_bool()),
    ("netplay", "relay_only", |v: &Value| v.is_bool()),
];

/// Removes `[netplay]` from a non-global layer (logged: the value has no effect).
pub(crate) fn strip_netplay_section(layer: &mut Table, source: &Path) {
    if layer.remove(NETPLAY_SECTION).is_some() {
        tracing::warn!(layer = %source.display(),
            "[netplay] is a global setting; ignored in this layer");
    }
}

#[cfg(test)]
#[cfg(feature = "fs")]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use super::*;
    use crate::{resolve_config, ConfigLayerPaths};

    #[test]
    fn value_rules() {
        assert!(valid_server_url("wss://netplay.nostyra.com"));
        assert!(valid_server_url("http://127.0.0.1:3536"));
        for bad in ["", "ftp://x", "wss://", "wss://a b", "wss://x?token=1"] {
            assert!(!valid_server_url(bad), "{bad}");
        }
        for good in ["auto", "0", "10"] {
            assert!(valid_input_delay(good), "{good}");
        }
        for bad in ["11", "-1", "Auto", ""] {
            assert!(!valid_input_delay(bad), "{bad}");
        }
        assert!(valid_nickname("") && valid_nickname("Ngọc Ánh"));
        assert!(!valid_nickname(&"x".repeat(33)) && !valid_nickname("a\nb"));
    }

    #[test]
    fn netplay_comes_from_the_global_layer_only() {
        let temp = tempfile::tempdir().unwrap();
        let layers = ConfigLayerPaths::new(temp.path(), "sys", "core", Path::new("Game.rom"), "");
        let write = |path: &Path, text: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        assert_eq!(
            resolve_config(&layers, None).netplay,
            NetplayConfig::default()
        );
        write(
            &layers.global,
            "[netplay]\nnickname = \"Vivi\"\ninput_delay = \"3\"\nhud = false\n\
             server_url = \"not a url\"\nrelay_only = true\n",
        );
        write(&layers.game, "[netplay]\nnickname = \"Other\"\n");
        let config = resolve_config(&layers, None).netplay;
        assert_eq!(config.nickname, "Vivi", "game layer ignored");
        assert_eq!(config.input_delay, "3");
        assert!(!config.hud && config.relay_only);
        assert_eq!(
            config.server_url, NETPLAY_DEFAULT_SERVER_URL,
            "invalid → default"
        );
    }
}
