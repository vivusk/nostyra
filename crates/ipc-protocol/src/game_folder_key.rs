//! Names of a game's own folder (saves) and file (game config layer):
//! `<rom key> [<content id>]`, e.g. `Chrono Trigger (USA) [6B47BB75]`.
//!
//! The content id (first 8 hex digits of the game's content sha1, the hash the
//! library scanner stores) is what finds the entry: lookup matches the ` [ID]`
//! suffix only, so renaming or moving the ROM, or renaming the folder's name part,
//! keeps the saves. The `<rom key>` part only tells people which game it is.
//! Entries written before content ids (plain `<rom key>`) are renamed on first use.

use std::path::{Path, PathBuf};

use crate::rom_key::rom_key;

/// Hex digits of the content sha1 kept in names.
pub const CONTENT_ID_LEN: usize = 8;

/// Upper-case content id from a sha1 hex string; `None` when it is not hex or too short.
pub fn content_id(content_sha1: &str) -> Option<String> {
    let id = content_sha1.get(..CONTENT_ID_LEN)?;
    id.chars()
        .all(|c| c.is_ascii_hexdigit())
        .then(|| id.to_ascii_uppercase())
}

/// The content id at the end of an entry name (`ext` included, e.g. `.toml`), if any.
pub fn content_id_of_name<'a>(name: &'a str, ext: &str) -> Option<&'a str> {
    let stem = strip_suffix_ignore_case(name, ext)?;
    let tail = stem.get(stem.len().checked_sub(CONTENT_ID_LEN + 3)?..)?;
    let id = tail.strip_prefix(" [")?.strip_suffix(']')?;
    id.chars().all(|c| c.is_ascii_hexdigit()).then_some(id)
}

/// `<rom key> [ID]<ext>` for a new entry.
pub fn game_entry_name(rom_path: &Path, id: &str, ext: &str) -> String {
    format!("{} [{id}]{ext}", rom_key(rom_path))
}

/// The entry in `parent` whose name ends in ` [id]<ext>` (first by name when
/// several do, so both processes pick the same one).
pub fn find_game_entry(parent: &Path, id: &str, ext: &str) -> Option<PathBuf> {
    let mut names: Vec<String> = std::fs::read_dir(parent)
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| content_id_of_name(name, ext).is_some_and(|i| i.eq_ignore_ascii_case(id)))
        .collect();
    names.sort();
    names.into_iter().next().map(|name| parent.join(name))
}

/// Where the game's entry lives in `parent` (see the module doc). Renames a legacy
/// `<rom key><ext>` entry to the new name; if that rename fails the legacy path is
/// returned, so the game still uses its data. Without a usable sha1 the legacy name
/// is used as before.
pub fn resolve_game_entry(
    parent: &Path,
    rom_path: &Path,
    content_sha1: &str,
    ext: &str,
) -> PathBuf {
    let legacy = parent.join(format!("{}{ext}", rom_key(rom_path)));
    let Some(id) = content_id(content_sha1) else {
        return legacy;
    };
    if let Some(found) = find_game_entry(parent, &id, ext) {
        return found;
    }
    let new = parent.join(game_entry_name(rom_path, &id, ext));
    if legacy.exists() && std::fs::rename(&legacy, &new).is_err() {
        return legacy;
    }
    new
}

fn strip_suffix_ignore_case<'a>(name: &'a str, suffix: &str) -> Option<&'a str> {
    let split = name.len().checked_sub(suffix.len())?;
    (name.is_char_boundary(split) && name[split..].eq_ignore_ascii_case(suffix))
        .then(|| &name[..split])
}

#[cfg(test)]
#[path = "game_folder_key_tests.rs"]
mod tests;
