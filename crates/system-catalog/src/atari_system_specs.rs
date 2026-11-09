//! Atari cartridge systems: 2600 (Stella), 7800 (ProSystem), Lynx (Handy). No BIOS
//! needed, no SAVE_RAM (Lynx EEPROM: a file Handy writes in the save folder).
//!
//! Formats: 2600 `.a26` and 7800 `.a78` / Lynx `.lnx` (headered, matched against the
//! headerless No-Intro entries too, `HeaderRule::MagicHeader`), Lynx `.lyx` (raw) and
//! `.o` / `.bll` (BS93 homebrew). The headerless `.bin` that 2600 and 7800 share
//! with Mega Drive and disc images is listed only when the file is in the system's
//! DAT ([`ExtCheck::DatHash`]).

use crate::disc_probe::Media;
use crate::per_extension_probe::{ExtCheck, ExtensionCheck};
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};
use crate::web_core_specs::{WEB_HANDY, WEB_PROSYSTEM, WEB_STELLA2014};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * 1024;

/// A78 header: `ATARI7800` at 1, 128 bytes (ProSystem and rcheevos check these 9).
const A78_MAGIC: &[u8] = b"ATARI7800";
const A78_HEADER_BYTES: usize = 128;
/// LNX header: `LYNX` + NUL at 0, 64 bytes (rcheevos compares the 5 bytes).
const LNX_MAGIC: &[u8] = b"LYNX\0";
const LNX_HEADER_BYTES: usize = 64;

/// Atari 2600: `.a26` (No-Intro), `.bin` (GoodTools, DAT-listed only). 2 KiB ..
/// 1 MiB (No-Intro: 2..64 KiB; modern bankswitch schemes go larger).
pub const ATARI2600: SystemSpec = SystemSpec {
    id: "atari2600",
    display_name: "Atari 2600",
    short_label: "2600",
    media: Media::Cartridge,
    extensions: &["a26", "bin"],
    probe: ContentProbe::PerExtension(&[
        ExtensionCheck {
            exts: &["a26"],
            check: ExtCheck::Any,
        },
        ExtensionCheck {
            exts: &["bin"],
            check: ExtCheck::DatHash,
        },
    ]),
    header_rule: HeaderRule::None,
    min_bytes: 2 * KIB,
    max_bytes: MIB,
    default_core_id: "stella",
    dat_file: "Atari - 2600.dat",
    metadat_file: "Atari - 2600.dat",
    thumbnails_repo: "Atari_-_2600",
    thumbnails_folder: "Atari - 2600",
    // Stella: B = Fire; console switches on port 0 (each button *sets* a switch).
    pad_actions: &[
        "b", "select", "start", "l", "l2", "r", "r2", "l3", "r3", "up", "down", "left", "right",
    ],
    pad_labels: &[
        ("b", "Fire"),
        ("select", "Game Select"),
        ("start", "Game Reset"),
        ("l", "Left difficulty A"),
        ("l2", "Left difficulty B"),
        ("r", "Right difficulty A"),
        ("r2", "Right difficulty B"),
        ("l3", "Colour"),
        ("r3", "B/W"),
    ],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(25),
    web_core: Some(WEB_STELLA2014),
};

/// Atari 7800: `.a78` (with or without the A78 header: GoodTools sets name headerless
/// dumps `.a78`; ProSystem reads both), `.bin` (headerless No-Intro dumps, DAT-listed
/// only). 4 KiB .. 1 MiB of ROM, + the header.
pub const ATARI7800: SystemSpec = SystemSpec {
    id: "atari7800",
    display_name: "Atari 7800",
    short_label: "7800",
    media: Media::Cartridge,
    extensions: &["a78", "bin"],
    probe: ContentProbe::PerExtension(&[
        ExtensionCheck {
            exts: &["a78"],
            check: ExtCheck::Any,
        },
        ExtensionCheck {
            exts: &["bin"],
            check: ExtCheck::DatHash,
        },
    ]),
    header_rule: HeaderRule::MagicHeader {
        offset: 1,
        magic: A78_MAGIC,
        len: A78_HEADER_BYTES,
    },
    min_bytes: 4 * KIB,
    max_bytes: MIB + A78_HEADER_BYTES as u64,
    default_core_id: "prosystem",
    dat_file: "Atari - 7800.dat",
    metadat_file: "Atari - 7800.dat",
    thumbnails_repo: "Atari_-_7800",
    thumbnails_folder: "Atari - 7800",
    // ProSystem: Reset is X; the difficulty switches are momentary (A while held).
    pad_actions: &[
        "b", "a", "x", "select", "start", "l", "r", "up", "down", "left", "right",
    ],
    pad_labels: &[
        ("b", "Button 1"),
        ("a", "Button 2"),
        ("x", "Reset"),
        ("select", "Select"),
        ("start", "Pause"),
        ("l", "Left difficulty (hold = A)"),
        ("r", "Right difficulty (hold = A)"),
    ],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(51),
    web_core: Some(WEB_PROSYSTEM),
};

/// Atari Lynx: `.lnx` (with or without the LNX header: GoodTools names raw dumps
/// `.lnx`; Handy reads both), `.lyx` (raw cartridge), `.o` / `.bll`
/// (BS93 homebrew; without the magic a C object file, ignored silently). 16 B ..
/// 1 MiB of data, + the header.
pub const LYNX: SystemSpec = SystemSpec {
    id: "lynx",
    display_name: "Atari Lynx",
    short_label: "Lynx",
    media: Media::Cartridge,
    extensions: &["lnx", "lyx", "o", "bll"],
    probe: ContentProbe::PerExtension(&[
        ExtensionCheck {
            exts: &["lnx", "lyx"],
            check: ExtCheck::Any,
        },
        ExtensionCheck {
            exts: &["o", "bll"],
            check: ExtCheck::Magic {
                offset: 6,
                bytes: b"BS93",
                missing: "not a Lynx BS93 program",
                quiet: true,
            },
        },
    ]),
    header_rule: HeaderRule::MagicHeader {
        offset: 0,
        magic: LNX_MAGIC,
        len: LNX_HEADER_BYTES,
    },
    min_bytes: 16,
    max_bytes: MIB + LNX_HEADER_BYTES as u64,
    default_core_id: "handy",
    dat_file: "Atari - Lynx.dat",
    metadat_file: "Atari - Lynx.dat",
    thumbnails_repo: "Atari_-_Lynx",
    thumbnails_folder: "Atari - Lynx",
    // Handy: no SELECT (it would rotate the screen; rotation comes from the cart).
    pad_actions: &["a", "b", "l", "r", "start", "up", "down", "left", "right"],
    pad_labels: &[
        ("a", "A"),
        ("b", "B"),
        ("l", "Option 1"),
        ("r", "Option 2"),
        ("start", "Pause"),
    ],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(13),
    web_core: Some(WEB_HANDY),
};
