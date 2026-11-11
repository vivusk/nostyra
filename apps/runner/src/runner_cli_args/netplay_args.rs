//! `--netplay host|join|watch` and its options (M20). The room itself (server url,
//! room id, token, ICE servers, player cap) comes from the launcher in the env JSON
//! `NOSTYRA_NETPLAY_SESSION`, never as arguments. Hidden test flags drive the
//! loopback test: autostart, a scripted held input, a frame limit, a hidden window.

use clap::{Args, ValueEnum};

use crate::runner_held_input::pattern_parser::parse_pattern;
use crate::runner_held_input::HeldInput;

/// Seat requested on the command line (must equal the env's role).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum NetplayRoleArg {
    Host,
    Join,
    Watch,
}

/// `--netplay-input-delay auto|N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDelayArg {
    Auto,
    Frames(u8),
}

/// Largest manual input delay (frames), same bound as `[netplay] input_delay`.
pub const MAX_INPUT_DELAY: u8 = 10;

#[derive(Debug, Clone, Default, Args)]
pub struct NetplayArgs {
    /// Play online: `host` a room, `join` it as a player or `watch` as a spectator
    /// (the launcher sets `NOSTYRA_NETPLAY_SESSION`).
    #[arg(long = "netplay", value_name = "ROLE")]
    pub role: Option<NetplayRoleArg>,

    /// Host only: players of the room (2-4, must equal the room's cap).
    #[arg(long = "netplay-players", requires = "role", required_if_eq("role", "host"),
          value_parser = clap::value_parser!(u8).range(2..=4))]
    pub players: Option<u8>,

    /// Name shown to the other peers.
    #[arg(long = "netplay-nickname", requires = "role")]
    pub nickname: Option<String>,

    /// Input delay in frames (`auto` = from the lobby ping).
    #[arg(long = "netplay-input-delay", requires = "role", value_parser = parse_input_delay)]
    pub input_delay: Option<InputDelayArg>,

    /// Prefer TURN relay (best effort: matchbox cannot force it).
    #[arg(long = "netplay-relay-only", requires = "role")]
    pub relay_only: bool,

    /// In-game HUD (ping, rollbacks, relay); warnings show either way.
    #[arg(long = "netplay-hud", requires = "role", value_parser = parse_on_off)]
    pub hud: Option<bool>,

    /// Test-only (host): send Start once the room is full and this many spectators
    /// joined (default 0).
    #[arg(long = "netplay-autostart", hide = true, requires = "role", num_args = 0..=1,
          default_missing_value = "0", value_name = "SPECTATORS")]
    pub autostart: Option<u8>,

    /// Test-only: local input = this held pattern (`none`, `b`, `joypad:<id>`,
    /// `stick-right`, `trigger-right`) instead of keyboard / pads.
    #[arg(long = "netplay-scripted-input", hide = true, requires = "role", value_parser = parse_pattern)]
    pub scripted_input: Option<HeldInput>,

    /// Test-only: stop local input at frame N, wait until N is confirmed (+ settle),
    /// print `ram_crc=<hex> frames=N` and quit.
    #[arg(long = "netplay-frames-limit", hide = true, requires = "role",
          value_parser = clap::value_parser!(i32).range(1..))]
    pub frames_limit: Option<i32>,

    /// Test-only: keep the game window hidden.
    #[arg(long = "window-hidden", hide = true)]
    pub window_hidden: bool,
}

fn parse_input_delay(value: &str) -> Result<InputDelayArg, String> {
    if value == "auto" {
        return Ok(InputDelayArg::Auto);
    }
    match value.parse::<u8>() {
        Ok(frames) if frames <= MAX_INPUT_DELAY => Ok(InputDelayArg::Frames(frames)),
        _ => Err(format!("expected `auto` or 0..={MAX_INPUT_DELAY}")),
    }
}

fn parse_on_off(value: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err("expected `on` or `off`".to_string()),
    }
}

impl NetplayArgs {
    /// HUD shown (default on).
    pub fn hud_on(&self) -> bool {
        self.hud.unwrap_or(true)
    }
}

impl InputDelayArg {
    /// The lobby's override (`None` = auto).
    pub fn frames(self) -> Option<usize> {
        match self {
            Self::Auto => None,
            Self::Frames(frames) => Some(usize::from(frames)),
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::runner_cli_args::RunnerCliArgs;

    fn parse(extra: &[&str]) -> Result<RunnerCliArgs, clap::Error> {
        let base = ["nostyra-runner", "--core", "c", "--rom", "r"];
        RunnerCliArgs::try_parse_from(base.iter().chain(extra))
    }

    #[test]
    fn netplay_flags_parse_and_validate() {
        let host = parse(&["--netplay", "host", "--netplay-players", "3"]).unwrap();
        assert_eq!(host.netplay.role, Some(NetplayRoleArg::Host));
        assert_eq!(host.netplay.players, Some(3));
        assert!(host.netplay.hud_on(), "HUD on by default");
        assert!(parse(&["--netplay", "host"]).is_err(), "host needs players");
        assert!(parse(&["--netplay", "host", "--netplay-players", "5"]).is_err());
        assert!(parse(&["--netplay", "join"]).is_ok());
        assert!(
            parse(&["--netplay-players", "2"]).is_err(),
            "needs --netplay"
        );
        assert!(parse(&["--netplay", "spy"]).is_err());
        let plain = parse(&[]).unwrap();
        assert_eq!(plain.netplay.role, None);
        assert!(!plain.netplay.window_hidden);
    }

    #[test]
    fn hidden_test_flags_parse() {
        let args = parse(&[
            "--netplay",
            "join",
            "--netplay-autostart",
            "1",
            "--netplay-scripted-input",
            "joypad:13",
            "--netplay-frames-limit",
            "600",
            "--netplay-input-delay",
            "3",
            "--netplay-hud",
            "off",
            "--window-hidden",
        ])
        .unwrap();
        let np = &args.netplay;
        assert_eq!(np.autostart, Some(1));
        assert_eq!(np.scripted_input, Some(HeldInput::Joypad(13)));
        assert_eq!(np.frames_limit, Some(600));
        assert_eq!(np.input_delay, Some(InputDelayArg::Frames(3)));
        assert!(!np.hud_on() && np.window_hidden);
        let bare = parse(&["--netplay", "watch", "--netplay-autostart"]).unwrap();
        assert_eq!(bare.netplay.autostart, Some(0));
        let auto = parse(&["--netplay", "join", "--netplay-input-delay", "auto"]).unwrap();
        assert_eq!(
            auto.netplay.input_delay.and_then(InputDelayArg::frames),
            None
        );
        for bad in [
            &["--netplay", "join", "--netplay-input-delay", "11"][..],
            &["--netplay", "join", "--netplay-scripted-input", "joypad:x"],
            &["--netplay", "join", "--netplay-frames-limit", "0"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
