//! Sega 8/16-bit cartridge systems (Mega Drive / Genesis, Master System, Game Gear,
//! SG-1000), all played by Genesis Plus GX. The core picks the system by file
//! extension (`.sms` / `.gg` / `.sg`, anything else is a Mega Drive), so 8-bit
//! files are extension-only and keep their own extensions.

use crate::disc_probe::Media;
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};

/// Mega Drive / Genesis: No-Intro `.md`, GoodGen `.bin` / `.gen` / `.smd`. `.bin`
/// is shared with disc images and `.md` with Markdown documents, so both need
/// `SEGA` at 0x100 (0x101 on a few dumps); a `.md` without it is no ROM at all
/// (`README.md`). `.smd` (copier layout, no magic at 0x100) and `.gen` are
/// extension-only; SMD files hash as their raw image. 512 B .. 10 MiB (the core
/// truncates anything larger).
pub const GENESIS: SystemSpec = SystemSpec {
    id: "genesis",
    display_name: "Mega Drive / Genesis",
    short_label: "MD",
    media: Media::Cartridge,
    extensions: &["md", "gen", "smd", "bin"],
    probe: ContentProbe::MagicForExtensions {
        exts: &["bin"],
        quiet_exts: &["md"],
        offsets: &[0x100, 0x101],
        bytes: b"SEGA",
        missing: "missing Mega Drive header",
    },
    header_rule: HeaderRule::MegaDriveSmd,
    min_bytes: 512,
    max_bytes: 10 * 1024 * 1024,
    default_core_id: "genesis_plus_gx",
    dat_file: "Sega - Mega Drive - Genesis.dat",
    metadat_file: "Sega - Mega Drive - Genesis.dat",
    thumbnails_repo: "Sega_-_Mega_Drive_-_Genesis",
    thumbnails_folder: "Sega - Mega Drive - Genesis",
    // Genesis Plus GX RetroPad map: Y/B/A = A/B/C, L/X/R = X/Y/Z, Select = Mode.
    pad_actions: &[
        "y", "b", "a", "l", "x", "r", "select", "start", "up", "down", "left", "right",
    ],
    pad_labels: &[
        ("y", "A"),
        ("b", "B"),
        ("a", "C"),
        ("l", "X"),
        ("x", "Y"),
        ("r", "Z"),
        ("select", "Mode"),
    ],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(1),
};

/// Master System / Mark III: `.sms`, 1 KiB .. 4 MiB (8-bit carts are at most 1 MiB;
/// homebrew and Korean mappers go larger). Start is the console's Pause button.
pub const SMS: SystemSpec = SystemSpec {
    id: "sms",
    display_name: "Master System",
    short_label: "SMS",
    media: Media::Cartridge,
    extensions: &["sms"],
    probe: ContentProbe::Any,
    header_rule: HeaderRule::None,
    min_bytes: 1024,
    max_bytes: 4 * 1024 * 1024,
    default_core_id: "genesis_plus_gx",
    dat_file: "Sega - Master System - Mark III.dat",
    metadat_file: "Sega - Master System - Mark III.dat",
    thumbnails_repo: "Sega_-_Master_System_-_Mark_III",
    thumbnails_folder: "Sega - Master System - Mark III",
    pad_actions: SEGA_8BIT_PAD_ACTIONS,
    pad_labels: &[("b", "1"), ("a", "2"), ("start", "Pause")],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(11),
};

/// Game Gear: `.gg`, same limits as the Master System.
pub const GG: SystemSpec = SystemSpec {
    id: "gg",
    display_name: "Game Gear",
    short_label: "GG",
    media: Media::Cartridge,
    extensions: &["gg"],
    probe: ContentProbe::Any,
    header_rule: HeaderRule::None,
    min_bytes: 1024,
    max_bytes: 4 * 1024 * 1024,
    default_core_id: "genesis_plus_gx",
    dat_file: "Sega - Game Gear.dat",
    metadat_file: "Sega - Game Gear.dat",
    thumbnails_repo: "Sega_-_Game_Gear",
    thumbnails_folder: "Sega - Game Gear",
    pad_actions: SEGA_8BIT_PAD_ACTIONS,
    pad_labels: SEGA_8BIT_PAD_LABELS,
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(15),
};

/// SG-1000: `.sg`, same limits as the Master System.
pub const SG1000: SystemSpec = SystemSpec {
    id: "sg1000",
    display_name: "SG-1000",
    short_label: "SG",
    media: Media::Cartridge,
    extensions: &["sg"],
    probe: ContentProbe::Any,
    header_rule: HeaderRule::None,
    min_bytes: 1024,
    max_bytes: 4 * 1024 * 1024,
    default_core_id: "genesis_plus_gx",
    dat_file: "Sega - SG-1000.dat",
    metadat_file: "Sega - SG-1000.dat",
    thumbnails_repo: "Sega_-_SG-1000",
    thumbnails_folder: "Sega - SG-1000",
    pad_actions: SEGA_8BIT_PAD_ACTIONS,
    pad_labels: SEGA_8BIT_PAD_LABELS,
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(33),
};

/// Button 1 / Button 2 / Start (Pause on the Master System) + D-pad.
const SEGA_8BIT_PAD_ACTIONS: &[&str] = &["b", "a", "start", "up", "down", "left", "right"];
const SEGA_8BIT_PAD_LABELS: &[(&str, &str)] = &[("b", "1"), ("a", "2")];
