//! Removes staged content left by runners that crashed (`<data>/temp/staged-content/
//! <pid>/`, see `libretro_host` staging). Best effort at start: only folders older
//! than [`STALE_AGE`], so a parallel runner (CLI, tests) between staging and
//! `retro_load_game` keeps its file; the launcher itself runs one game at a time.
//! The guard protects the load window only: a folder is not touched after load, so
//! a parallel runner started an hour later may delete a live session's file (harmless
//! for cartridge cores, which read the content once at load; `Drop` then just logs).

use std::path::Path;
use std::time::{Duration, SystemTime};

/// Staging folders younger than this are left alone.
pub const STALE_AGE: Duration = Duration::from_secs(60 * 60);

/// Deletes every folder in `root` except `own` whose modification time is more than
/// `max_age` ago. Failures are logged, never fatal.
pub fn remove_stale(root: &Path, own: &Path, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return; // nothing staged yet
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        if path == own {
            continue;
        }
        let age = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok());
        if age.is_some_and(|age| age >= max_age) {
            match std::fs::remove_dir_all(&path) {
                Ok(()) => tracing::info!(path = %path.display(), "stale staged content removed"),
                Err(err) => {
                    tracing::warn!(path = %path.display(), %err, "cannot remove staged content")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_own_and_young_folders_removes_old_ones() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-stage-cleanup-")
            .tempdir()
            .unwrap();
        let own = temp.path().join("100");
        let other = temp.path().join("200");
        for dir in [&own, &other] {
            std::fs::create_dir(dir).unwrap();
            std::fs::write(dir.join("content.md"), b"x").unwrap();
        }
        // Young (age < 1 h): kept.
        remove_stale(temp.path(), &own, STALE_AGE);
        assert!(own.exists() && other.exists());
        // Old enough (any age >= 0): removed, own folder kept.
        remove_stale(temp.path(), &own, Duration::ZERO);
        assert!(own.exists());
        assert!(!other.exists());
        // Missing root: no-op.
        remove_stale(&temp.path().join("none"), &own, Duration::ZERO);
    }

    /// Dolphin writes its `User/` tree (memory cards, NAND, shader cache) into the
    /// game's save folder (`GET_SAVE_DIRECTORY`): the cleanup only ever looks below
    /// the staging root, never there.
    #[test]
    fn never_touches_a_games_user_folder() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-stage-user-")
            .tempdir()
            .unwrap();
        let data = temp.path();
        let rom = data.join("Game.iso");
        let saves = ipc_protocol::saves_dir_for_game(data, system_catalog::GC.id, &rom, "ABCDEF01");
        let card = saves.join("User").join("GC").join("USA").join("Card A");
        std::fs::create_dir_all(&card).unwrap();
        std::fs::write(card.join("01-GALE-SuperSmashBros0110290334.gci"), b"x").unwrap();
        let staged = ipc_protocol::staged_content_dir(data);
        std::fs::create_dir_all(staged.join("1")).unwrap();
        remove_stale(&staged, &staged.join("2"), Duration::ZERO);
        assert!(!staged.join("1").exists(), "stale staging removed");
        assert!(card.join("01-GALE-SuperSmashBros0110290334.gci").is_file());
        assert!(!saves.starts_with(&staged));
    }
}
