//! Resume flow: what Play should do for a game (`resume.mode` + its resume
//! point and warnings). Continue = `play_game(gameId, "auto")`; Start fresh =
//! plain `play_game` (the resume point is never deleted by it). In achievement
//! hardcore the resume point is not offered at all (Play starts fresh; the save
//! list shows why, `achievements_mode`).

use config_layers::ResumeMode;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::command_error::CommandResult;
use crate::commands::achievements_mode::hardcore_for_game;
use crate::commands::blocking::run_blocking;
use crate::commands::game_data_target::GameDataTarget;
use crate::commands::resume_point::{
    current_patch_crc32, read_resume_point, ResumePoint, ResumeWarning,
};
use crate::commands::state_load_rule::StateLoadRule;

/// Everything the Play button needs to choose between plain launch, direct
/// Continue and the Resume dialog.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeInfo {
    /// Effective `resume.mode` for this game.
    pub mode: ResumeMode,
    /// `auto.state` with a readable sidecar exists and may be loaded (never in
    /// achievement hardcore).
    pub available: bool,
    /// Unix seconds of the resume point.
    pub created_at: Option<i64>,
    /// Absolute `auto.png` path for `assetUrl`.
    pub thumbnail_path: Option<String>,
    pub warnings: Vec<ResumeWarning>,
    /// Why the resume point cannot be loaded (`state-incompatible`): Continue hidden.
    pub incompatible_reason: Option<String>,
    /// The next launch runs in achievement hardcore (Play shows "resume disabled").
    pub hardcore: bool,
}

#[tauri::command]
pub async fn get_resume_info(app: AppHandle, game_id: i64) -> CommandResult<ResumeInfo> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let data_root = &state.paths.data_root;
        let target = GameDataTarget::load(&state, game_id)?;
        let (config, _) = target.resolve_with_warnings(data_root);
        let point = match target.saves_dir(data_root) {
            Some(dir) => {
                let rule = StateLoadRule::for_game(&state, &target)?;
                read_resume_point(&dir, current_patch_crc32(&target.rom_path), &rule)
            }
            None => None,
        };
        let hardcore = hardcore_for_game(data_root, &target);
        Ok(resume_info(config.resume.mode, point, hardcore))
    })
    .await
}

/// The Play facts for `point`; hardcore hides it (Continue never offered).
fn resume_info(mode: ResumeMode, point: Option<ResumePoint>, hardcore: bool) -> ResumeInfo {
    let point = point.filter(|_| !hardcore);
    ResumeInfo {
        mode,
        available: point.is_some(),
        created_at: point.as_ref().map(|p| p.created_at),
        thumbnail_path: point.as_ref().and_then(|p| p.thumbnail_path.clone()),
        incompatible_reason: point.as_ref().and_then(|p| p.incompatible_reason.clone()),
        warnings: point.map(|p| p.warnings).unwrap_or_default(),
        hardcore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardcore_hides_the_resume_point() {
        let point = ResumePoint {
            created_at: 5,
            thumbnail_path: None,
            warnings: Vec::new(),
            incompatible_reason: None,
        };
        let soft = resume_info(ResumeMode::Always, Some(point.clone()), false);
        assert!(soft.available && soft.created_at == Some(5));
        let hard = resume_info(ResumeMode::Always, Some(point), true);
        assert!(!hard.available && hard.created_at.is_none() && hard.hardcore);
    }
}
