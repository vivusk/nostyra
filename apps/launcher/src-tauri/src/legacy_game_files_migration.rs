//! Startup move of per-game files written before content ids: a save folder
//! `saves/<system>/<rom key>/` or game config `config/game/<system>/<rom key>.toml`
//! gets its game's id appended (`<rom key> [<ID>]`), so it keeps working after the
//! ROM is renamed. Runs before any Play, on every start (cheap when nothing is left).
//!
//! Never deletes or overwrites: a legacy entry claimed by one game is renamed; one
//! shared by several different ROMs with the same file name (old keys were file-name
//! based) is copied to each and left in place; anything doubtful stays and is logged.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use ipc_protocol::{content_id, content_id_of_name, find_game_entry, rom_key};
use storage_db::GameFilesKey;

/// What one start did, for the log and tests.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LegacyMigrationSummary {
    pub renamed: usize,
    pub copied: usize,
    /// Readable reasons for entries left as they are.
    pub kept: Vec<String>,
}

/// Reads every game's key from the DB and migrates; errors are only logged (the
/// runner also renames a legacy folder on first use).
pub fn migrate_legacy_game_files_at_startup(data_root: &Path, conn: &storage_db::Connection) {
    let keys = match storage_db::game_files_keys(conn, None) {
        Ok(keys) => keys,
        Err(err) => {
            tracing::warn!(%err, "legacy game files not migrated");
            return;
        }
    };
    let summary = migrate_legacy_game_files(data_root, &keys);
    if summary.renamed + summary.copied > 0 {
        tracing::info!(
            summary.renamed,
            summary.copied,
            "legacy game files migrated"
        );
    }
    for reason in &summary.kept {
        tracing::warn!(%reason, "legacy game file kept in place");
    }
}

/// Migrates the legacy entries of `keys`' games under `data_root`.
pub fn migrate_legacy_game_files(
    data_root: &Path,
    keys: &[GameFilesKey],
) -> LegacyMigrationSummary {
    // (system, rom key) → distinct content ids of the games naming that legacy entry.
    let mut claims: BTreeMap<(&str, String), BTreeSet<String>> = BTreeMap::new();
    for key in keys {
        if let Some(id) = content_id(&key.sha1) {
            claims
                .entry((key.platform_id.as_str(), rom_key(Path::new(&key.rom_path))))
                .or_default()
                .insert(id);
        }
    }
    let mut summary = LegacyMigrationSummary::default();
    for ((system, name), ids) in &claims {
        let kinds = [
            (ipc_protocol::saves_system_dir(data_root, system), ""),
            (
                ipc_protocol::game_config_system_dir(data_root, system),
                ipc_protocol::GAME_CONFIG_EXT,
            ),
        ];
        for (parent, ext) in &kinds {
            migrate_entry(parent, name, ext, ids, &mut summary);
        }
    }
    summary
}

/// One legacy entry `<parent>/<name><ext>` claimed by `ids`.
fn migrate_entry(
    parent: &Path,
    name: &str,
    ext: &str,
    ids: &BTreeSet<String>,
    summary: &mut LegacyMigrationSummary,
) {
    let legacy = parent.join(format!("{name}{ext}"));
    // A ROM named `… [ABCD1234]` would look like a new entry: leave it alone.
    if !legacy.exists() || content_id_of_name(&format!("{name}{ext}"), ext).is_some() {
        return;
    }
    let targets: Vec<_> = ids
        .iter()
        .filter(|id| find_game_entry(parent, id, ext).is_none())
        .map(|id| parent.join(format!("{name} [{id}]{ext}")))
        .collect();
    if targets.is_empty() {
        summary.kept.push(format!(
            "{} (the game already has its own)",
            legacy.display()
        ));
        return;
    }
    if ids.len() == 1 {
        match std::fs::rename(&legacy, &targets[0]) {
            Ok(()) => summary.renamed += 1,
            Err(err) => summary
                .kept
                .push(format!("cannot rename {}: {err}", legacy.display())),
        }
        return;
    }
    for target in &targets {
        match copy_to_new_entry(&legacy, target) {
            Ok(()) => summary.copied += 1,
            Err(err) => summary.kept.push(format!(
                "cannot copy {} to {}: {err}",
                legacy.display(),
                target.display()
            )),
        }
    }
    summary.kept.push(format!(
        "{} (shared by {} different ROMs with this file name; copied to each)",
        legacy.display(),
        ids.len()
    ));
}

/// Copies a file, or a folder tree, to `to`, which must not exist (an existing
/// `to` is an error and is left untouched). A copy that fails half-way removes
/// what it created, so the next start retries.
fn copy_to_new_entry(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_file() {
        let mut dst = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)?;
        let copied = std::fs::File::open(from).and_then(|mut src| io::copy(&mut src, &mut dst));
        drop(dst);
        return copied.map(|_| ()).inspect_err(|_| {
            let _ = std::fs::remove_file(to);
        });
    }
    std::fs::create_dir(to)?;
    copy_tree(from, to).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(to);
    })
}

/// Copies the contents of folder `from` into the (new, empty) folder `to`.
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type()?.is_dir() {
            std::fs::create_dir(&dst)?;
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "legacy_game_files_migration_tests.rs"]
mod tests;
