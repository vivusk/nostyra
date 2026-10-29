//! What the launcher hands to one runner process: paths, extra arguments and extra
//! environment (RetroAchievements login, M15). Secrets travel only in `env` (never
//! argv, which other users can see in process lists) and never reach a log: the
//! manual `Debug` prints env names but redacts token values.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// Env var whose value `Debug` hides (mirrors `achievements::TOKEN_ENV`; this crate
/// stays free of the achievements dependency).
pub const REDACTED_ENV: &str = "NOSTYRA_RA_TOKEN";

/// Everything needed to start a game.
#[derive(Clone)]
pub struct RunnerLaunch {
    pub runner: PathBuf,
    pub core: PathBuf,
    /// Must be `games.rom_path`: the pending session file is matched by it.
    pub rom: PathBuf,
    /// Registry system id of the game (runner `--system`).
    pub system: String,
    pub data_dir: PathBuf,
    /// Appended after the standard args (tests: `--headless-frames N`).
    pub extra_args: Vec<OsString>,
    /// Extra environment for the runner process (empty = inherit only).
    pub env: Vec<(OsString, OsString)>,
}

impl std::fmt::Debug for RunnerLaunch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let env: Vec<(&OsStr, &OsStr)> = self
            .env
            .iter()
            .map(|(name, value)| {
                let shown: &OsStr = if name == REDACTED_ENV {
                    OsStr::new("<redacted>")
                } else {
                    value
                };
                (name.as_os_str(), shown)
            })
            .collect();
        f.debug_struct("RunnerLaunch")
            .field("runner", &self.runner)
            .field("core", &self.core)
            .field("rom", &self.rom)
            .field("system", &self.system)
            .field("data_dir", &self.data_dir)
            .field("extra_args", &self.extra_args)
            .field("env", &env)
            .finish()
    }
}

/// Runner arguments: `--core --rom --system --data-dir --ipc [extra]`.
pub fn runner_args(launch: &RunnerLaunch) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "--core".into(),
        launch.core.clone().into(),
        "--rom".into(),
        launch.rom.clone().into(),
        "--system".into(),
        launch.system.clone().into(),
        "--data-dir".into(),
        launch.data_dir.clone().into(),
        "--ipc".into(),
    ];
    args.extend(launch.extra_args.iter().cloned());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_the_token_but_keeps_other_env() {
        let launch = RunnerLaunch {
            runner: "runner".into(),
            core: "core".into(),
            rom: "rom".into(),
            system: "snes".into(),
            data_dir: "data".into(),
            extra_args: Vec::new(),
            env: vec![
                ("NOSTYRA_RA_USER".into(), "Tester".into()),
                (REDACTED_ENV.into(), "tok-secret-123".into()),
            ],
        };
        let text = format!("{launch:?}");
        assert!(!text.contains("tok-secret-123"), "{text}");
        assert!(
            text.contains("<redacted>") && text.contains("Tester"),
            "{text}"
        );
        assert!(!runner_args(&launch)
            .iter()
            .any(|arg| arg.to_string_lossy().contains("tok-secret")));
    }
}
