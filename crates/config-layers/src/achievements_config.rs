//! `[achievements]` section (M15 RetroAchievements). **Global layer only**: it is
//! account-wide (one RA login, one mode), so the table is stripped from the system,
//! core and game layers before merging (a game layer `hardcore = true` is ignored).
//! `hardcore` is honoured only while `achievements::hardcore_ui_enabled()` (the
//! runner checks; this crate only stores the value).

use std::path::Path;

use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::known_key_checks::KeyCheck;

/// Section name of the achievement settings.
pub const ACHIEVEMENTS_SECTION: &str = "achievements";

/// RetroAchievements settings (launcher Settings, global layer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AchievementsConfig {
    /// Achievements run whenever a login exists (off: no session, zero cost).
    pub enabled: bool,
    /// Hardcore mode, read at launch (ignored while the hardcore UI is hidden).
    pub hardcore: bool,
    /// Unlock chime (overlay, phase 05).
    pub sound: bool,
    /// Leaderboard start / submit popups (overlay, phase 05).
    pub leaderboard_popups: bool,
    /// A soft-patched cartridge RA does not know (e.g. a fan translation) retries with
    /// the original file's hash, always softcore (phase 03 of the offline follow-up).
    pub patched_rom_fallback: bool,
}

impl Default for AchievementsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hardcore: false,
            sound: true,
            leaderboard_popups: true,
            patched_rom_fallback: true,
        }
    }
}

/// Value checks of the `[achievements]` keys.
pub(crate) const ACHIEVEMENTS_KEY_CHECKS: [KeyCheck; 5] = [
    ("achievements", "enabled", |v: &Value| v.is_bool()),
    ("achievements", "hardcore", |v: &Value| v.is_bool()),
    ("achievements", "sound", |v: &Value| v.is_bool()),
    ("achievements", "leaderboard_popups", |v: &Value| {
        v.is_bool()
    }),
    ("achievements", "patched_rom_fallback", |v: &Value| {
        v.is_bool()
    }),
];

/// Removes `[achievements]` from a non-global layer (logged: the value has no effect).
pub(crate) fn strip_global_only(layer: &mut Table, source: &Path) {
    if layer.remove(ACHIEVEMENTS_SECTION).is_some() {
        tracing::warn!(layer = %source.display(),
            "[achievements] is a global setting; ignored in this layer");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use crate::{resolve_config, ConfigLayerPaths};

    #[test]
    fn achievements_come_from_the_global_layer_only() {
        let temp = tempfile::tempdir().unwrap();
        let layers = ConfigLayerPaths::new(
            temp.path(),
            "sys",
            "core",
            std::path::Path::new("Game.rom"),
            "",
        );
        let write = |path: &std::path::Path, text: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        assert_eq!(
            resolve_config(&layers, None).achievements,
            super::AchievementsConfig::default()
        );
        assert!(super::AchievementsConfig::default().patched_rom_fallback);
        write(
            &layers.global,
            "[achievements]\nsound = false\nenabled = 3\npatched_rom_fallback = false\n",
        );
        write(&layers.system, "[achievements]\nenabled = false\n");
        write(
            &layers.game,
            "[achievements]\nhardcore = true\npatched_rom_fallback = true\n[rewind]\nenabled = true\n",
        );
        let config = resolve_config(&layers, None);
        assert!(!config.achievements.sound, "global value applies");
        assert!(
            config.achievements.enabled,
            "invalid global value → default; system ignored"
        );
        assert!(!config.achievements.hardcore, "game layer hardcore ignored");
        assert!(
            !config.achievements.patched_rom_fallback,
            "global value applies; game layer ignored"
        );
        assert!(
            config.rewind.enabled,
            "rest of the game layer still applies"
        );
    }
}
