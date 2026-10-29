//! `[resume]` section: the runner writes a resume point on clean exits
//! (`auto_save`), the launcher decides whether Play offers it (`mode`).

use serde::{Deserialize, Serialize};
use toml::Value;

use crate::known_key_checks::KeyCheck;

/// Resume-point settings (global Settings and per-game Detail).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResumeConfig {
    /// Runner saves the `auto` pseudo-slot on every clean exit.
    pub auto_save: bool,
    /// What the launcher's Play does when a resume point exists.
    pub mode: ResumeMode,
}

impl Default for ResumeConfig {
    fn default() -> Self {
        Self {
            auto_save: true,
            mode: ResumeMode::default(),
        }
    }
}

/// Launcher behaviour when a resume point exists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResumeMode {
    /// Show the Continue / Start fresh dialog.
    #[default]
    Ask,
    /// Continue directly unless the resume point has warnings.
    Always,
    /// Always start fresh (games with good in-game saves).
    Never,
}

/// Value checks of the `[resume]` keys.
pub(crate) const RESUME_KEY_CHECKS: [KeyCheck; 2] = [
    ("resume", "auto_save", |v| v.is_bool()),
    ("resume", "mode", |v: &Value| {
        v.as_str()
            .is_some_and(|s| ["ask", "always", "never"].contains(&s))
    }),
];
