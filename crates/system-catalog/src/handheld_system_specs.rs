//! Nintendo handheld systems (Game Boy, Game Boy Color, Game Boy Advance), all
//! played by mGBA.

use crate::disc_probe::Media;
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};

/// Game Boy: `.gb` (the No-Intro naming; `.gbc` is its own system), 32 KiB .. 8 MiB.
/// mGBA only loads files with the first 4 Nintendo logo bytes at 0x104.
pub const GB: SystemSpec = SystemSpec {
    id: "gb",
    display_name: "Game Boy",
    short_label: "GB",
    media: Media::Cartridge,
    extensions: &["gb"],
    probe: GB_LOGO_PROBE,
    header_rule: HeaderRule::None,
    min_bytes: 32 * 1024,
    max_bytes: 8 * 1024 * 1024,
    default_core_id: "mgba",
    dat_file: "Nintendo - Game Boy.dat",
    metadat_file: "Nintendo - Game Boy.dat",
    thumbnails_repo: "Nintendo_-_Game_Boy",
    thumbnails_folder: "Nintendo - Game Boy",
    pad_actions: GB_PAD_ACTIONS,
    pad_labels: &[],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(4),
};

/// Game Boy Color: `.gbc`, same cartridge header and limits as the Game Boy.
pub const GBC: SystemSpec = SystemSpec {
    id: "gbc",
    display_name: "Game Boy Color",
    short_label: "GBC",
    media: Media::Cartridge,
    extensions: &["gbc"],
    probe: GB_LOGO_PROBE,
    header_rule: HeaderRule::None,
    min_bytes: 32 * 1024,
    max_bytes: 8 * 1024 * 1024,
    default_core_id: "mgba",
    dat_file: "Nintendo - Game Boy Color.dat",
    metadat_file: "Nintendo - Game Boy Color.dat",
    thumbnails_repo: "Nintendo_-_Game_Boy_Color",
    thumbnails_folder: "Nintendo - Game Boy Color",
    pad_actions: GB_PAD_ACTIONS,
    pad_labels: &[],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(6),
};

/// Game Boy Advance: `.gba`, 1 KiB (homebrew) .. 32 MiB. mGBA wants an ARM branch
/// at the entry point (byte 3 = `0xEA`); its extra `0xB2 = 0x96` check is not
/// probed (one magic per probe; every retail dump has it).
pub const GBA: SystemSpec = SystemSpec {
    id: "gba",
    display_name: "Game Boy Advance",
    short_label: "GBA",
    media: Media::Cartridge,
    extensions: &["gba"],
    probe: ContentProbe::Magic {
        offset: 3,
        bytes: &[0xEA],
        missing: "missing GBA header",
    },
    header_rule: HeaderRule::None,
    min_bytes: 1024,
    max_bytes: 32 * 1024 * 1024,
    default_core_id: "mgba",
    dat_file: "Nintendo - Game Boy Advance.dat",
    metadat_file: "Nintendo - Game Boy Advance.dat",
    thumbnails_repo: "Nintendo_-_Game_Boy_Advance",
    thumbnails_folder: "Nintendo - Game Boy Advance",
    pad_actions: &[
        "b", "a", "select", "start", "up", "down", "left", "right", "l", "r",
    ],
    pad_labels: &[],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(5),
};

/// GB / GBC cartridge check: the first 4 logo bytes at 0x104 (what mGBA checks).
const GB_LOGO_PROBE: ContentProbe = ContentProbe::Magic {
    offset: 0x104,
    bytes: &[0xCE, 0xED, 0x66, 0x66],
    missing: "missing Game Boy header",
};

const GB_PAD_ACTIONS: &[&str] = &["b", "a", "select", "start", "up", "down", "left", "right"];
