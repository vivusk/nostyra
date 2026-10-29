//! Snapshot → Achievements tab rows: unofficial rows dropped, unlocked first (newest
//! first), locked in set order; each row points at the cached badge file that exists
//! (the `_lock` variant for locked rows, else the colour badge greyed by the UI).

use std::path::Path;

use achievements::{badge_path, game_icon_path, GameSnapshot};
use serde::Serialize;

/// Snapshot rows with this category are unofficial (not counted, not shown).
const UNOFFICIAL_CATEGORY: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementsHeader {
    pub title: String,
    /// Cached game icon (absolute path for `assetUrl`), if downloaded.
    pub icon_path: Option<String>,
    /// Mode of the session that wrote the snapshot.
    pub hardcore: bool,
    pub updated_at: i64,
    pub unlocked: u32,
    pub total: u32,
    pub points_unlocked: u32,
    pub points_total: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementRow {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub unlocked: bool,
    pub unlocked_hardcore: bool,
    /// Unix seconds; 0 while locked.
    pub unlock_time: i64,
    /// `"5/10"`; empty when not measured.
    pub measured_progress: String,
    /// Cached badge file, if present (locked: the `_lock` variant, else the colour one).
    pub badge_path: Option<String>,
    /// The UI greys the badge itself (locked row without a `_lock` file).
    pub badge_grayscale: bool,
}

/// The tab rows of `snapshot`, sorted and with their badge files resolved.
pub fn rows(data_root: &Path, snapshot: &GameSnapshot) -> Vec<AchievementRow> {
    let mut rows: Vec<AchievementRow> = snapshot
        .achievements
        .iter()
        .filter(|a| a.category != UNOFFICIAL_CATEGORY)
        .map(|a| {
            let (badge_path, badge_grayscale) = badge_for(data_root, &a.badge, a.unlocked);
            AchievementRow {
                id: a.id,
                title: a.title.clone(),
                description: a.description.clone(),
                points: a.points,
                unlocked: a.unlocked,
                unlocked_hardcore: a.unlocked_hardcore,
                unlock_time: a.unlock_time,
                measured_progress: a.measured_progress.clone(),
                badge_path,
                badge_grayscale,
            }
        })
        .collect();
    // Stable: locked rows keep the set order.
    rows.sort_by_key(|row| {
        (
            !row.unlocked,
            if row.unlocked { -row.unlock_time } else { 0 },
        )
    });
    rows
}

/// Existing badge file for a row, and whether the UI must grey it.
fn badge_for(data_root: &Path, badge: &str, unlocked: bool) -> (Option<String>, bool) {
    let existing = |locked: bool| {
        badge_path(data_root, badge, locked)
            .filter(|path| path.is_file())
            .map(|path| path.to_string_lossy().into_owned())
    };
    if unlocked {
        return (existing(false), false);
    }
    match existing(true) {
        Some(path) => (Some(path), false),
        None => (existing(false), true),
    }
}

/// Header counts over the shown `rows` (unofficial ones excluded).
pub fn header(
    data_root: &Path,
    snapshot: &GameSnapshot,
    rows: &[AchievementRow],
) -> AchievementsHeader {
    let unlocked = rows.iter().filter(|row| row.unlocked);
    AchievementsHeader {
        title: snapshot.title.clone(),
        icon_path: game_icon_path(data_root, &snapshot.icon_badge)
            .filter(|path| path.is_file())
            .map(|path| path.to_string_lossy().into_owned()),
        hardcore: snapshot.hardcore,
        updated_at: snapshot.updated_at,
        unlocked: count(unlocked.clone().count()),
        total: count(rows.len()),
        points_unlocked: unlocked.map(|row| row.points).sum(),
        points_total: rows.iter().map(|row| row.points).sum(),
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
