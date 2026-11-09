//! Sony PlayStation 2, played by PCEE2 (PCSX2 libretro, OpenGL).

use crate::disc_probe::{DiscSignature, Media};
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};

/// Sony PlayStation 2: DVD / CD dumps as `.iso`, raw `.bin` (+ `.cue`), CD or DVD
/// `.chd`, `.cso` / `.zso` (CISO / LZ4 ZSO) and `.m3u` multi-disc playlists.
/// Recognised by content, never by extension: every one of these names is shared
/// with PlayStation / PSP / GameCube / Wii. The probe wants the ISO9660 system id
/// `PLAYSTATION` plus a `BOOT2 = cdrom0:\…` line in the root `SYSTEM.CNF` (PS1 discs
/// have `BOOT` and the licence text, so they are claimed first). 9 GiB covers a
/// dual-layer DVD-9 (8.5 GB). Pad = the DualShock 2 as PCEE2 describes it: the PS1
/// set plus the stick clicks (L3 / R3) and both analog sticks; L2 / R2 are digital
/// on the RetroPad (button pressure is a separate analog index nobody exposes) and
/// the host has no rumble interface, so the core's request is refused silently.
pub const PS2: SystemSpec = SystemSpec {
    id: "ps2",
    display_name: "PlayStation 2",
    short_label: "PS2",
    media: Media::Disc,
    extensions: &["iso", "bin", "cue", "chd", "cso", "zso", "m3u"],
    probe: ContentProbe::Disc(DiscSignature::PlayStation2),
    header_rule: HeaderRule::None,
    min_bytes: 16,
    max_bytes: 9 * 1024 * 1024 * 1024,
    default_core_id: "pcee2",
    dat_file: "Sony - PlayStation 2.dat",
    metadat_file: "Sony - PlayStation 2.dat",
    thumbnails_repo: "Sony_-_PlayStation_2",
    thumbnails_folder: "Sony - PlayStation 2",
    // PCEE2 RetroPad map: Cross = b, Circle = a, Square = y, Triangle = x; L1 / R1 =
    // l / r, L2 / R2 = l2 / r2, L3 / R3 = l3 / r3; sticks = analog index 0 / 1.
    #[rustfmt::skip]
    pad_actions: &[
        "b", "a", "y", "x", "select", "start", "up", "down", "left", "right", "l", "r", "l2", "r2",
        "l3", "r3",
        "lstick_up", "lstick_down", "lstick_left", "lstick_right",
        "rstick_up", "rstick_down", "rstick_left", "rstick_right",
    ],
    pad_labels: &[
        ("b", "Cross"),
        ("a", "Circle"),
        ("y", "Square"),
        ("x", "Triangle"),
        ("l", "L1"),
        ("r", "R1"),
        ("l2", "L2"),
        ("r2", "R2"),
        ("l3", "L3"),
        ("r3", "R3"),
        ("select", "Select"),
        ("start", "Start"),
        ("lstick_up", "Left Stick Up"),
        ("lstick_down", "Left Stick Down"),
        ("lstick_left", "Left Stick Left"),
        ("lstick_right", "Left Stick Right"),
        ("rstick_up", "Right Stick Up"),
        ("rstick_down", "Right Stick Down"),
        ("rstick_left", "Right Stick Left"),
        ("rstick_right", "Right Stick Right"),
    ],
    analog_sticks: AnalogSticks::Both,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(21),
    web_core: None,
};

#[cfg(test)]
#[path = "ps2_system_spec_tests.rs"]
mod tests;
