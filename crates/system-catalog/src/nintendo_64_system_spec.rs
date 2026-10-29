//! Nintendo 64, played by Mupen64Plus-Next (OpenGL hardware rendering).

use crate::disc_probe::Media;
use crate::n64_byte_order::{N64_MAGIC, V64_MAGIC, Z64_MAGIC};
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};

/// Nintendo 64: `.z64` (big-endian, No-Intro), `.v64` (16-bit swapped) and `.n64`
/// (32-bit little-endian). The first word decides the byte order, not the
/// extension; every order hashes as the `.z64` image. 4 KiB (header + IPL3 end at
/// 0x1000) .. 64 MiB (the largest DAT entry). `.bin` (shared with Mega Drive and
/// PlayStation), `.ndd` (64DD) and archives are not listed.
pub const NINTENDO_64: SystemSpec = SystemSpec {
    id: "n64",
    display_name: "Nintendo 64",
    short_label: "N64",
    media: Media::Cartridge,
    extensions: &["z64", "n64", "v64"],
    probe: ContentProbe::MagicAny {
        offset: 0,
        patterns: &[&Z64_MAGIC, &V64_MAGIC, &N64_MAGIC],
        missing: "missing N64 header",
    },
    header_rule: HeaderRule::N64ByteOrder,
    min_bytes: 4 * 1024,
    max_bytes: 64 * 1024 * 1024,
    default_core_id: "mupen64plus-next",
    dat_file: "Nintendo - Nintendo 64.dat",
    metadat_file: "Nintendo - Nintendo 64.dat",
    thumbnails_repo: "Nintendo_-_Nintendo_64",
    thumbnails_folder: "Nintendo - Nintendo 64",
    // Mupen64Plus-Next RetroPad map: B/Y = A/B, L2 = Z, L/R = shoulders, left
    // stick = the N64 stick, right stick = C-buttons (core option C1-C4).
    pad_actions: &[
        "b",
        "y",
        "l2",
        "l",
        "r",
        "start",
        "up",
        "down",
        "left",
        "right",
        "lstick_up",
        "lstick_down",
        "lstick_left",
        "lstick_right",
        "rstick_up",
        "rstick_down",
        "rstick_left",
        "rstick_right",
    ],
    pad_labels: &[
        ("b", "A"),
        ("y", "B"),
        ("l2", "Z"),
        ("l", "L"),
        ("r", "R"),
        ("lstick_up", "Stick Up"),
        ("lstick_down", "Stick Down"),
        ("lstick_left", "Stick Left"),
        ("lstick_right", "Stick Right"),
        ("rstick_up", "C-Up"),
        ("rstick_down", "C-Down"),
        ("rstick_left", "C-Left"),
        ("rstick_right", "C-Right"),
    ],
    analog_sticks: AnalogSticks::Both,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(2),
};
