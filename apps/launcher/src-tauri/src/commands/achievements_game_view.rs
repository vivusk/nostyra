//! Game Detail → Achievements tab data: the runner's per-game snapshot
//! (`<data>/achievements/games/<content sha1>.json`, written while a logged-in session
//! runs) plus which cached badge files exist. The launcher has no live RA client and
//! fetches nothing here (offline-first, plan Q10).

use std::path::Path;

use achievements::{load_game_snapshot, GameSnapshot};
use config_layers::global_config_path;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::command_error::CommandResult;
use crate::commands::achievements_snapshot_rows::{
    header, rows, AchievementRow, AchievementsHeader,
};
use crate::commands::blocking::run_blocking;
use crate::commands::game_data_target::GameDataTarget;
use crate::commands::input_config_view::resolve_global_only;

/// Which empty state (or the list) the tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AchievementsStatus {
    /// The system has no RetroAchievements support (GameCube / Wii).
    Unsupported,
    LoggedOut,
    /// Never played while logged in: no snapshot yet.
    NotLoaded,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAchievementsView {
    pub status: AchievementsStatus,
    pub system_name: String,
    /// `achievements.enabled` (global): off → the runner starts no session.
    pub enabled: bool,
    pub game: Option<AchievementsHeader>,
    /// Unlocked first (newest first), then locked in set order.
    pub achievements: Vec<AchievementRow>,
}

#[tauri::command]
pub async fn game_achievements(
    app: AppHandle,
    game_id: i64,
) -> CommandResult<GameAchievementsView> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let target = GameDataTarget::load(&state, game_id)?;
        let data_root = &state.paths.data_root;
        let logged_in = matches!(achievements::load_credentials(data_root), Ok(Some(_)));
        let (config, _) = resolve_global_only(&global_config_path(data_root));
        Ok(build_game_view(
            data_root,
            &target,
            logged_in,
            config.achievements.enabled,
        ))
    })
    .await
}

/// The tab's view for one game, without Tauri.
pub fn build_game_view(
    data_root: &Path,
    target: &GameDataTarget,
    logged_in: bool,
    enabled: bool,
) -> GameAchievementsView {
    let system = system_catalog::system(&target.system);
    let mut view = GameAchievementsView {
        status: AchievementsStatus::NotLoaded,
        system_name: system.map_or_else(|| target.system.clone(), |s| s.short_label.to_owned()),
        enabled,
        game: None,
        achievements: Vec::new(),
    };
    if !system.is_some_and(|s| s.ra_console_id.is_some()) {
        view.status = AchievementsStatus::Unsupported;
    } else if !logged_in {
        view.status = AchievementsStatus::LoggedOut;
    } else if let Some(snapshot) = read_snapshot(data_root, &target.content_sha1) {
        view.status = AchievementsStatus::Ready;
        view.achievements = rows(data_root, &snapshot);
        view.game = Some(header(data_root, &snapshot, &view.achievements));
    }
    view
}

/// The snapshot for a stored sha1; only plain hex names are looked up (no path tricks).
fn read_snapshot(data_root: &Path, content_sha1: &str) -> Option<GameSnapshot> {
    let plain_hex = ipc_protocol::content_id(content_sha1).is_some()
        && content_sha1.chars().all(|c| c.is_ascii_hexdigit());
    if !plain_hex {
        return None;
    }
    load_game_snapshot(data_root, content_sha1).unwrap_or_else(|err| {
        tracing::warn!(%err, "unreadable achievement snapshot");
        None
    })
}

#[cfg(test)]
#[path = "achievements_game_view_tests.rs"]
mod tests;
