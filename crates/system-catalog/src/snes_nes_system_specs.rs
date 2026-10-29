//! Super Nintendo and NES entries of the registry (`system_specs`).

use crate::disc_probe::Media;
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};

/// Super Nintendo: `.sfc` / `.smc`, 32 KiB .. 16 MiB, optional 512-byte copier header.
pub const SNES: SystemSpec = SystemSpec {
    id: "snes",
    display_name: "Super Nintendo",
    short_label: "SNES",
    media: Media::Cartridge,
    extensions: &["sfc", "smc"],
    probe: ContentProbe::Any,
    header_rule: HeaderRule::SnesCopier512,
    min_bytes: 32 * 1024,
    max_bytes: 16 * 1024 * 1024,
    default_core_id: "snes9x",
    dat_file: "Nintendo - Super Nintendo Entertainment System.dat",
    metadat_file: "Nintendo - Super Nintendo Entertainment System.dat",
    thumbnails_repo: "Nintendo_-_Super_Nintendo_Entertainment_System",
    thumbnails_folder: "Nintendo - Super Nintendo Entertainment System",
    pad_actions: &[
        "b", "y", "select", "start", "up", "down", "left", "right", "a", "x", "l", "r",
    ],
    pad_labels: &[],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(3),
};

/// NES / Famicom: `.nes` with an iNES header (libretro NES cores need it for the
/// mapper), 8 KiB .. 8 MiB whole file.
pub const NES: SystemSpec = SystemSpec {
    id: "nes",
    display_name: "Nintendo Entertainment System",
    short_label: "NES",
    media: Media::Cartridge,
    extensions: &["nes"],
    probe: ContentProbe::Magic {
        offset: 0,
        bytes: b"NES\x1A",
        missing: "missing iNES header",
    },
    header_rule: HeaderRule::INes,
    min_bytes: 8 * 1024,
    max_bytes: 8 * 1024 * 1024,
    default_core_id: "mesen",
    dat_file: "Nintendo - Nintendo Entertainment System.dat",
    metadat_file: "Nintendo - Nintendo Entertainment System.dat",
    thumbnails_repo: "Nintendo_-_Nintendo_Entertainment_System",
    thumbnails_folder: "Nintendo - Nintendo Entertainment System",
    pad_actions: &["b", "a", "select", "start", "up", "down", "left", "right"],
    pad_labels: &[],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(7),
};
