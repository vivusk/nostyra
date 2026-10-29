//! One log line at start for disc systems with BIOS files: which BIOS the core will
//! boot for this disc's region (a real file from `<data>/system/`, or the built-in
//! HLE BIOS), the same answer the launcher's Settings → BIOS shows.

use std::path::Path;

use disc_image::{is_m3u_ext, open_data_track, playlist_entries, region, DiscRegion};
use system_catalog::{core_for_system, scan_firmware, BiosChoice, SystemSpec};

/// Logs the BIOS choice for `rom` (no-op for cores without BIOS files, and for cores
/// whose BIOS option the runner answers: `bios_option_switch` logs those).
pub fn log_bios_choice(system: &SystemSpec, system_dir: &Path, rom: &Path) {
    let scanned = |c: &&system_catalog::CoreSpec| !c.firmware.is_empty() && c.bios_switch.is_none();
    let Some(core) = core_for_system(system.id).filter(scanned) else {
        return;
    };
    let region = disc_region(rom).unwrap_or("US");
    match scan_firmware(system_dir, core).choice_for(region) {
        BiosChoice::Real { file_name } => {
            tracing::info!("{} BIOS for {region}: {file_name}", system.display_name);
        }
        BiosChoice::Hle => {
            tracing::info!(
                "{} BIOS for {region}: built-in HLE BIOS",
                system.display_name
            );
        }
    }
}

/// Region code (`JP` / `US` / `EU`) from the disc's licence text; `None` when the
/// image cannot tell (`.pbp`, no licence sector).
fn disc_region(rom: &Path) -> Option<&'static str> {
    let ext = rom.extension().and_then(|e| e.to_str()).unwrap_or("");
    let disc = if is_m3u_ext(ext) {
        playlist_entries(rom).ok()?.into_iter().next()?
    } else {
        rom.to_path_buf()
    };
    let mut reader = open_data_track(&disc).ok()?;
    Some(match region(reader.as_mut()).ok()?? {
        DiscRegion::Japan => "JP",
        DiscRegion::America => "US",
        DiscRegion::Europe => "EU",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disc_region_comes_from_its_licence_text() {
        let roms = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms");
        assert_eq!(disc_region(&roms.join("nostyra-test-psx.cue")), Some("US"));
        assert_eq!(disc_region(&roms.join("nostyra-test-psx.m3u")), Some("US"));
        assert_eq!(disc_region(&roms.join("nostyra-test.sfc")), None);
    }
}
