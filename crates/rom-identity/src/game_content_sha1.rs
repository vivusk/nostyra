//! The sha1 a scan stores for a game (`games.sha1`), computed for one file without
//! a scan: the runner keys a game's save folder on it when the launcher did not
//! pass it (`--content-sha1`). Same bytes as `build_scanned_game` hashes.

use std::path::Path;

use system_catalog::SystemSpec;

use crate::game_identity::{game_identity, IdentityError};

/// Upper-case hex sha1 of the game at `path` (cartridge: header rule of `system`;
/// disc: the system's identity track of the first disc; GameCube / Wii: SHA1 of the
/// canonical game-ID text, no disc byte read past the header). The message says why it failed.
pub fn game_content_sha1(path: &Path, system: &SystemSpec) -> Result<String, String> {
    match game_identity(path, system) {
        Ok(identity) => Ok(identity.hashes.sha1),
        Err(IdentityError::ZipSystemMismatch) => {
            Err("ROM in ZIP does not match the selected system".into())
        }
        Err(IdentityError::Read(message)) => Err(message),
    }
}
