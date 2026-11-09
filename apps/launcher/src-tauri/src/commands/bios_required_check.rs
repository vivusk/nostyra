//! Play refused before the runner starts when the game's core cannot boot without a
//! user BIOS dump and the BIOS folder has no valid one (PS2 / PCEE2: no HLE BIOS).
//! The runner would refuse too (`ps2_bios_check`, exit 4), but that ends as a generic
//! "game ended" toast; a typed `bios_missing` error lets the UI offer the file picker
//! (`BiosRequiredDialog`) instead, as `core_missing` offers the core download.

use std::path::Path;

use system_catalog::{scan_firmware, CoreSpec};

use super::bios_region_views::has_hle_fallback;
use crate::command_error::{CommandError, CommandResult};

/// `Ok` when `core` boots without a BIOS file (HLE fallback) or `system_dir` holds a
/// valid dump for it; else the `bios_missing` error naming `core`.
pub fn require_bios(
    system_dir: &Path,
    core: &'static CoreSpec,
    platform_id: &str,
) -> CommandResult<()> {
    if has_hle_fallback(core) {
        return Ok(());
    }
    if scan_firmware(system_dir, core).valid().next().is_some() {
        return Ok(());
    }
    Err(CommandError::bios_missing(core, platform_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use system_catalog::{firmware_dir, ps2_bios_test_image::ps2_bios_image, PCEE2, PCSX_REARMED};

    fn temp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("nostyra-bios-required-")
            .tempdir()
            .unwrap()
    }

    #[test]
    fn empty_ps2_bios_folder_is_a_typed_bios_missing_error() {
        let dir = temp();
        let err = require_bios(dir.path(), &PCEE2, "ps2").unwrap_err();
        assert_eq!(err.kind, "bios_missing");
        assert_eq!(err.core_id, Some("pcee2"));
        assert!(err.message.contains("PS2"), "{}", err.message);
    }

    #[test]
    fn a_valid_ps2_dump_lifts_the_refusal() {
        let dir = temp();
        let folder = firmware_dir(dir.path(), &PCEE2);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("scph39001.bin"),
            ps2_bios_image("0160AC20020207", 0x2740),
        )
        .unwrap();
        require_bios(dir.path(), &PCEE2, "ps2").unwrap();
    }

    #[test]
    fn an_invalid_file_alone_still_refuses() {
        let dir = temp();
        let folder = firmware_dir(dir.path(), &PCEE2);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("junk.bin"), vec![0u8; 4 * 1024 * 1024]).unwrap();
        assert_eq!(
            require_bios(dir.path(), &PCEE2, "ps2").unwrap_err().kind,
            "bios_missing"
        );
    }

    #[test]
    fn hle_cores_never_block_play() {
        let dir = temp(); // no BIOS folder at all
        require_bios(dir.path(), &PCSX_REARMED, "psx").unwrap();
    }
}
