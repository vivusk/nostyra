//! The sha1 a scan stores for a game (`games.sha1`), computed for one file without
//! a scan: the runner keys a game's save folder on it when the launcher did not
//! pass it (`--content-sha1`). Same bytes as `build_scanned_game` hashes.

use std::path::Path;

use system_catalog::SystemSpec;

use crate::disc_game_identity::read_disc_identity;
use crate::rom_archive::{is_rom_archive, read_rom_archive};
use crate::rom_file_hasher::hash_rom_bytes;
use crate::rom_file_hasher::hash_rom_file;
use crate::serial_match_keys::serial_rule;

/// Upper-case hex sha1 of the game at `path` (cartridge: header rule of `system`;
/// disc: the system's identity track of the first disc; GameCube / Wii: SHA1 of the
/// canonical game-ID text, no disc byte read past the header). The message says why it failed.
pub fn game_content_sha1(path: &Path, system: &SystemSpec) -> Result<String, String> {
    if is_rom_archive(path) {
        let rom = read_rom_archive(path).map_err(|err| err.to_string())?;
        if rom.system != system {
            return Err("ROM in ZIP does not match the selected system".into());
        }
        return Ok(hash_rom_bytes(&rom.bytes, system).sha1);
    }
    if let Some(signature) = serial_rule(system) {
        return read_disc_identity(path, signature)
            .map(|identity| identity.hashes.sha1)
            .map_err(|err| err.to_string());
    }
    hash_rom_file(path, system)
        .map(|(hashes, _)| hashes.sha1)
        .map_err(|err| err.to_string())
}
