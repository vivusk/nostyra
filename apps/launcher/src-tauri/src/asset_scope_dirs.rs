//! Folders the webview may read through the asset protocol, opened at runtime (so
//! the paths match `ipc_protocol`'s data root on every OS): `<data>/artwork` (cached
//! covers; DB-relative art paths become absolute paths inside it), `<data>/saves`
//! (save-state thumbnails) and the achievement badge cache.

use std::path::{Component, Path};

use storage_db::Game;
use tauri::{AppHandle, Manager};

/// Creates the artwork folder (the scope needs an existing path), removes temp files
/// left by an interrupted fetch and allows the folder in the asset protocol scope.
pub fn allow_artwork_dir(app: &AppHandle, artwork_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(artwork_dir)?;
    let swept = artwork_cache::sweep_orphan_tmp(artwork_dir);
    if swept > 0 {
        tracing::info!(swept, "removed orphan artwork temp files");
    }
    app.asset_protocol_scope()
        .allow_directory(artwork_dir, true)?;
    Ok(())
}

/// Allows `<data>/saves` (created if missing) so save-state thumbnails load. Tauri's
/// runtime scope escapes glob characters, so a `**/*.png` pattern is impossible: the
/// whole folder is allowed. Accepted: the webview only runs our bundled UI and the
/// folder holds the user's own non-secret saves.
pub fn allow_saves_dir(app: &AppHandle, data_root: &Path) -> anyhow::Result<()> {
    let saves = ipc_protocol::saves_root(data_root);
    std::fs::create_dir_all(&saves)?;
    app.asset_protocol_scope().allow_directory(&saves, true)?;
    Ok(())
}

/// Allows `<data>/cache/achievement-badges` (created if missing): badges and game
/// icons the runner caches for the Achievements tab. Re-allowed at every start, so a
/// data folder change (which restarts the app) picks up the new location.
pub fn allow_badge_cache_dir(app: &AppHandle, data_root: &Path) -> anyhow::Result<()> {
    let badges = achievements::badge_cache_dir(data_root);
    std::fs::create_dir_all(&badges)?;
    app.asset_protocol_scope().allow_directory(&badges, true)?;
    Ok(())
}

/// Absolute path of a cached file, or `None` unless `relative` is one plain file name
/// (rows are written by the artwork job as `<id>-cover.jpg`; anything else is refused
/// so a bad row can never point the webview outside the artwork folder).
pub fn absolute_art_path(artwork_dir: &Path, relative: &str) -> Option<String> {
    let mut components = Path::new(relative).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) => {
            Some(artwork_dir.join(name).to_string_lossy().into_owned())
        }
        _ => None,
    }
}

/// The game with `cover_path` / `hero_path` made absolute (invalid paths dropped).
pub fn with_absolute_art(mut game: Game, artwork_dir: &Path) -> Game {
    game.cover_path = game
        .cover_path
        .and_then(|p| absolute_art_path(artwork_dir, &p));
    game.hero_path = game
        .hero_path
        .and_then(|p| absolute_art_path(artwork_dir, &p));
    game
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_file_names_become_absolute() {
        let dir = Path::new("data").join("artwork");
        assert_eq!(
            absolute_art_path(&dir, "12-cover.jpg"),
            Some(dir.join("12-cover.jpg").to_string_lossy().into_owned())
        );
        for bad in ["../nostyra.db", "index/snes-tree.json", "", "/etc/passwd"] {
            assert_eq!(absolute_art_path(&dir, bad), None, "{bad}");
        }
    }
}
