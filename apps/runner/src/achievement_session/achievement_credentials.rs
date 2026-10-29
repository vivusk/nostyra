//! Which RetroAchievements login a session uses: the launcher passes
//! `NOSTYRA_RA_USER` / `NOSTYRA_RA_TOKEN` in the runner's environment (never argv,
//! visible in process lists); without them (CLI use) `<data>/achievements/credentials.json`.
//! The env pair is read once at process start and removed from the runner's own
//! environment, so cores and their child processes never inherit the token.

use std::path::Path;

use achievements::{credentials_from_env, load_credentials, Credentials, TOKEN_ENV, USER_ENV};

/// Reads the launcher's login from the environment and clears it. Call at the very
/// start of `main`, before any other thread (core, SDL) exists.
pub fn take_env_credentials() -> Option<Credentials> {
    let credentials = credentials_from_env();
    std::env::remove_var(USER_ENV);
    std::env::remove_var(TOKEN_ENV);
    credentials
}

/// The session's login: the env pair, else the credentials file; `None` = logged
/// out (no session). An unreadable file is logged and treated as logged out.
pub fn session_credentials(env: Option<Credentials>, data_root: &Path) -> Option<Credentials> {
    if env.is_some() {
        return env;
    }
    match load_credentials(data_root) {
        Ok(credentials) => credentials,
        Err(err) => {
            tracing::warn!(%err, "unreadable achievements credentials file; achievements off");
            None
        }
    }
}
