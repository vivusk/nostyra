//! After a scan relinks a renamed ROM (same sha1, new file name), its per-game files
//! still carry the old ROM name: `saves/<system>/<old key> [<ID>]/` (SRAM, save
//! states, resume point) and `config/game/<system>/<old key> [<ID>].toml`. They are
//! found by `<ID>` anyway; this renames them to `<new key> [<ID>]` so the names keep
//! telling which game they belong to (a legacy `<old key>` entry gets its id on the
//! way). A name the user chose is kept. Never deletes or overwrites: anything
//! doubtful is skipped with a warning and the files stay where they are.

use std::io;
use std::path::{Path, PathBuf};

use ipc_protocol::{content_id, find_game_entry, game_entry_name, resolve_game_entry, rom_key};
use storage_db::{Connection, GameFilesKey, RelinkedGame};

/// What happened to one relinked game's files.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MigrationResult {
    pub game_id: i64,
    /// Destinations of the files/folders that were moved.
    pub moved: Vec<PathBuf>,
    /// Readable reasons for what was left in place.
    pub skipped: Vec<String>,
}

/// Renames the save folder and game config of every relink whose ROM key changed.
/// Callers hold the session lock for the whole call (Play cannot start mid-move)
/// and pass the running game, which is skipped.
pub fn migrate_relinked_game_files(
    data_root: &Path,
    conn: &Connection,
    relinked: &[RelinkedGame],
    running_game: Option<i64>,
) -> Vec<MigrationResult> {
    relinked
        .iter()
        .filter_map(|relink| {
            let keys = storage_db::game_files_keys(conn, Some(&relink.platform_id));
            migrate_one(data_root, keys, relink, running_game)
        })
        .inspect(log_result)
        .collect()
}

/// `None` when the ROM key did not change (folder-only move: nothing to do).
fn migrate_one(
    data_root: &Path,
    keys: storage_db::StorageResult<Vec<GameFilesKey>>,
    relink: &RelinkedGame,
    running_game: Option<i64>,
) -> Option<MigrationResult> {
    let (old, new) = (
        Path::new(&relink.old_rom_path),
        Path::new(&relink.new_rom_path),
    );
    let old_key = rom_key(old);
    if old_key == rom_key(new) {
        return None;
    }
    let mut result = MigrationResult {
        game_id: relink.game_id,
        ..MigrationResult::default()
    };
    if running_game == Some(relink.game_id) {
        result.skipped.push("the game is running".into());
        return Some(result);
    }
    let keys = match keys {
        Ok(keys) => keys,
        Err(err) => {
            result.skipped.push(format!("database error: {err}"));
            return Some(result);
        }
    };
    let Some((sha1, id)) = keys
        .iter()
        .find(|k| k.game_id == relink.game_id)
        .and_then(|own| Some((own.sha1.as_str(), content_id(&own.sha1)?)))
    else {
        result.skipped.push("the game has no content id".into());
        return Some(result);
    };
    let others: Vec<&GameFilesKey> = keys
        .iter()
        .filter(|k| k.game_id != relink.game_id)
        .collect();
    if others
        .iter()
        .any(|k| content_id(&k.sha1).as_deref() == Some(id.as_str()))
    {
        result
            .skipped
            .push("another copy of this game uses the same files".into());
        return Some(result);
    }
    // A legacy `<old key>` entry another game with that file name also used is left
    // to the startup migration (it copies shared entries).
    let old_key_shared = others
        .iter()
        .any(|k| rom_key(Path::new(&k.rom_path)) == old_key);
    let system = relink.platform_id.as_str();
    let kinds = [
        (ipc_protocol::saves_system_dir(data_root, system), ""),
        (
            ipc_protocol::game_config_system_dir(data_root, system),
            ipc_protocol::GAME_CONFIG_EXT,
        ),
    ];
    for (parent, ext) in &kinds {
        let current = if old_key_shared {
            find_game_entry(parent, &id, ext)
        } else {
            Some(resolve_game_entry(parent, old, sha1, ext))
        };
        let Some(current) = current.filter(|path| path.exists()) else {
            continue;
        };
        // Only names the app gave (`<old key> [ID]`) follow the ROM.
        if current.file_name() != Some(game_entry_name(old, &id, ext).as_ref()) {
            continue;
        }
        match move_if_free(&current, &parent.join(game_entry_name(new, &id, ext))) {
            Ok(to) => result.moved.push(to),
            Err(reason) => result.skipped.push(reason),
        }
    }
    Some(result)
}

/// Renames `from` to `to` when `to` does not exist. A case-only rename on a
/// case-insensitive file system sees `to` as taken and is skipped: the entry is
/// still found by its id, so nothing is lost.
fn move_if_free(from: &Path, to: &Path) -> Result<PathBuf, String> {
    if to.exists() {
        return Err(format!("{} already exists; kept both", to.display()));
    }
    std::fs::rename(from, to)
        .map(|()| to.to_path_buf())
        .map_err(|err: io::Error| format!("cannot move {}: {err}", from.display()))
}

fn log_result(result: &MigrationResult) {
    for to in &result.moved {
        tracing::info!(game_id = result.game_id, to = %to.display(), "renamed game: files renamed");
    }
    for reason in &result.skipped {
        tracing::warn!(game_id = result.game_id, %reason, "renamed game: files kept in place");
    }
}

#[cfg(test)]
#[path = "relinked_game_files_tests.rs"]
mod tests;
