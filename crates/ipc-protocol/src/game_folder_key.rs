//! Names of a game's own folder (saves) and file (game config layer):
//! `<rom key> [<content id>]`, e.g. `Chrono Trigger (USA) [6B47BB75]`.
//!
//! The content id (first 8 hex digits of the game's content sha1, the hash the
//! library scanner stores) is what finds the entry: lookup matches the ` [ID]`
//! suffix only, so renaming or moving the ROM, or renaming the folder's name part,
//! keeps the saves. The `<rom key>` part only tells people which game it is.
//! Entries written before content ids (plain `<rom key>`) are renamed on first use.

use std::path::Path;
#[cfg(feature = "native")]
use std::path::PathBuf;

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

/// Of the entry `names` of one folder, the one ending in ` [id]<ext>` (first by name
/// when several do, so every app picks the same one). Pure: the web edition passes
/// the names it listed through the File System Access API.
pub fn pick_game_entry_name<'a>(
    names: impl IntoIterator<Item = &'a str>,
    id: &str,
    ext: &str,
) -> Option<&'a str> {
    names
        .into_iter()
        .filter(|name| content_id_of_name(name, ext).is_some_and(|i| i.eq_ignore_ascii_case(id)))
        .min()
}

/// The entry in `parent` whose name ends in ` [id]<ext>` (see [`pick_game_entry_name`]).
#[cfg(feature = "native")]
pub fn find_game_entry(parent: &Path, id: &str, ext: &str) -> Option<PathBuf> {
    let names: Vec<String> = std::fs::read_dir(parent)
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    pick_game_entry_name(names.iter().map(String::as_str), id, ext).map(|name| parent.join(name))
}

/// Where the game's entry lives in `parent` (see the module doc). Renames a legacy
/// `<rom key><ext>` entry to the new name; if that rename fails the legacy path is
/// returned, so the game still uses its data. Without a usable sha1 the legacy name
/// is used as before.
#[cfg(feature = "native")]
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
#[cfg(feature = "native")]
#[path = "game_folder_key_tests.rs"]
mod tests;

#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn picks_the_first_name_with_the_id_suffix() {
        let names = [
            "Zelda [6B47BB75]",
            "Other [00000000]",
            "A copy [6b47bb75]",
            "Zelda",
        ];
        assert_eq!(
            pick_game_entry_name(names, "6B47BB75", ""),
            Some("A copy [6b47bb75]")
        );
        let files = ["Game [6B47BB75].toml", "Game [6B47BB75]"];
        assert_eq!(
            pick_game_entry_name(files, "6B47BB75", ".toml"),
            Some("Game [6B47BB75].toml")
        );
        assert_eq!(pick_game_entry_name(names, "FFFFFFFF", ""), None);
    }
}
