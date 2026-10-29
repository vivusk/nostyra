//! The Nintendo DS game code (`AMCE`): four ASCII characters at 0x0C of the header,
//! the serial the DS DAT lists. Identifies dumps the DAT has no hash for (hacks,
//! translations, trimmed or encrypted dumps) by their original game. Read with its
//! own small read, so it works when the hasher streamed the file.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use system_catalog::{ContentProbe, SystemSpec};

const CODE_OFFSET: usize = 0x0C;
const CODE_END: usize = CODE_OFFSET + 4;

/// The game code in `head` (the first bytes of a DS file); `None` when shorter or
/// not four ASCII letters/digits (homebrew `####`, an empty field).
pub(crate) fn nds_game_code(head: &[u8]) -> Option<String> {
    let code = head.get(CODE_OFFSET..CODE_END)?;
    code.iter()
        .all(u8::is_ascii_alphanumeric)
        .then(|| String::from_utf8_lossy(code).into_owned())
}

/// Serial a cartridge carries in its header: the DS game code for systems probed
/// by the DS header, else `None` (other cartridges are identified by hash only). A
/// read error also gives `None`: the hash already identified what it could.
pub(crate) fn cartridge_serial(path: &Path, system: &SystemSpec) -> Option<String> {
    if system.probe != ContentProbe::NdsHeader {
        return None;
    }
    let mut head = Vec::with_capacity(CODE_END);
    File::open(path)
        .and_then(|file| file.take(CODE_END as u64).read_to_end(&mut head))
        .ok()?;
    nds_game_code(&head)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(code: &[u8]) -> Vec<u8> {
        let mut head = vec![0u8; 0x10];
        head[CODE_OFFSET..CODE_OFFSET + code.len()].copy_from_slice(code);
        head
    }

    #[test]
    fn four_alphanumeric_characters_make_a_code() {
        assert_eq!(nds_game_code(&head(b"AMCE")).as_deref(), Some("AMCE"));
        assert_eq!(nds_game_code(&head(b"B2KJ")).as_deref(), Some("B2KJ"));
        assert_eq!(nds_game_code(&head(b"####")), None, "homebrew");
        assert_eq!(nds_game_code(&head(b"\0\0\0\0")), None);
        assert_eq!(nds_game_code(&head(b"AM E")), None);
        assert_eq!(nds_game_code(&head(b"AMCE")[..0x0F]), None, "short");
    }

    #[test]
    fn only_ds_header_systems_read_a_serial() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-nds-code-")
            .tempdir()
            .unwrap();
        let path = temp.path().join("game.nds");
        std::fs::write(&path, head(b"AMCE")).unwrap();
        assert_eq!(
            cartridge_serial(&path, &system_catalog::NDS).as_deref(),
            Some("AMCE")
        );
        assert_eq!(cartridge_serial(&path, &system_catalog::GBA), None);
        let missing = temp.path().join("missing.nds");
        assert_eq!(cartridge_serial(&missing, &system_catalog::NDS), None);
    }
}
