//! Hardcore stays fully coded but hidden until RetroAchievements approves Nostyra
//! (≥ 6 months public; Kodi precedent). While the flag is off the stored hardcore
//! setting is ignored (always softcore) and the Settings toggle is not shown.

/// The one switch that exposes the hardcore toggle. Flip only after RA approval.
pub const HARDCORE_UI_ENABLED: bool = false;

/// Env override for development and tests; read in debug builds only.
const HARDCORE_UI_ENV: &str = "NOSTYRA_RA_HARDCORE_UI";

/// True when the hardcore toggle (and therefore hardcore) may be used: the const, or
/// in debug builds `NOSTYRA_RA_HARDCORE_UI=1`.
pub fn hardcore_ui_enabled() -> bool {
    let env = std::env::var(HARDCORE_UI_ENV).ok();
    flag_with(HARDCORE_UI_ENABLED, cfg!(debug_assertions), env.as_deref())
}

fn flag_with(constant: bool, debug_build: bool, env: Option<&str>) -> bool {
    constant || (debug_build && env == Some("1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_only_in_debug_builds() {
        const {
            assert!(
                !HARDCORE_UI_ENABLED,
                "hardcore must stay hidden until RA approval"
            )
        };
        assert!(!flag_with(false, true, None));
        assert!(!flag_with(false, true, Some("0")));
        assert!(flag_with(false, true, Some("1")));
        assert!(
            !flag_with(false, false, Some("1")),
            "release ignores the env"
        );
        assert!(flag_with(true, false, None));
    }
}
