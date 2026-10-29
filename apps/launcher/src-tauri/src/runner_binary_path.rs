//! Locates the `nostyra-runner` executable the launcher spawns per game.
//!
//! M1 layout: the runner sits next to the launcher exe (shared cargo `target/` dir in dev).
//! Installed layout (bundled sidecar) is decided with release packaging.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Env var that overrides the runner location (debugging, tests).
pub const RUNNER_PATH_ENV: &str = "NOSTYRA_RUNNER";

/// Runner file name for the current OS.
pub const RUNNER_FILE_NAME: &str = if cfg!(windows) {
    "nostyra-runner.exe"
} else {
    "nostyra-runner"
};

/// Runner path: `NOSTYRA_RUNNER` if set (non-empty), else next to the launcher exe.
pub fn runner_binary_path() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let exe_dir = exe.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "launcher exe has no parent dir",
        )
    })?;
    Ok(resolve_runner_path(
        std::env::var_os(RUNNER_PATH_ENV),
        exe_dir,
    ))
}

/// Pure resolution rule, split out so it is testable without touching process env.
fn resolve_runner_path(env_override: Option<OsString>, exe_dir: &Path) -> PathBuf {
    match env_override {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => exe_dir.join(RUNNER_FILE_NAME),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_runner_next_to_launcher_exe() {
        let dir = Path::new("some").join("bin");
        assert_eq!(resolve_runner_path(None, &dir), dir.join(RUNNER_FILE_NAME));
    }

    #[test]
    fn env_override_wins() {
        let custom = PathBuf::from("custom").join("runner");
        let resolved = resolve_runner_path(Some(custom.clone().into_os_string()), Path::new("bin"));
        assert_eq!(resolved, custom);
    }

    #[test]
    fn empty_env_override_is_ignored() {
        let dir = Path::new("bin");
        assert_eq!(
            resolve_runner_path(Some(OsString::new()), dir),
            dir.join(RUNNER_FILE_NAME)
        );
    }
}
