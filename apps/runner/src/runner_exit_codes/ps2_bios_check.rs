//! PlayStation 2 launches without any valid BIOS dump are refused before the core
//! loads (exit 4), like DS dumps the core cannot play: PCEE2 would otherwise report a
//! successful load and show a silent black screen (M18 spike step 4), and the cores
//! have no HLE BIOS. The launcher shows the reason as for any refused ROM.

use std::path::Path;

use system_catalog::FirmwareReport;

use super::RomValidationError;

/// Reason shown when the BIOS folder has no valid dump.
pub(crate) const PS2_NO_BIOS: &str = "PlayStation 2 needs your BIOS dump — Settings → BIOS";

/// The refusal for `rom` when `report` (the core's BIOS folder) has no valid file.
pub(crate) fn no_bios_refusal(report: &FirmwareReport, rom: &Path) -> Option<RomValidationError> {
    if report.valid().next().is_some() {
        return None;
    }
    Some(RomValidationError::Unplayable {
        path: rom.to_path_buf(),
        reason: PS2_NO_BIOS,
    })
}

#[cfg(test)]
mod tests {
    use system_catalog::BiosFileStatus;

    use super::*;

    fn file(valid: bool) -> BiosFileStatus {
        BiosFileStatus {
            file_name: "scph39001.bin".into(),
            known: None,
            region: "US",
            valid,
            label: None,
            ps2_version: None,
        }
    }

    #[test]
    fn only_a_valid_file_lifts_the_refusal() {
        let rom = Path::new("game.iso");
        let empty = FirmwareReport { files: vec![] };
        let err = no_bios_refusal(&empty, rom).expect("refused");
        assert!(
            matches!(&err, RomValidationError::Unplayable { path, reason }
            if path == rom && *reason == PS2_NO_BIOS)
        );
        assert_eq!(err.to_string(), format!("{PS2_NO_BIOS}: game.iso"));
        let invalid = FirmwareReport {
            files: vec![file(false)],
        };
        assert!(no_bios_refusal(&invalid, rom).is_some());
        let valid = FirmwareReport {
            files: vec![file(false), file(true)],
        };
        assert!(no_bios_refusal(&valid, rom).is_none());
    }
}
