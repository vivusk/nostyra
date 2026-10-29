//! Sony PlayStation Portable, played by PPSSPP (OpenGL hardware rendering).

use crate::disc_probe::{DiscSignature, Media};
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};

/// Sony PSP: UMD dumps as `.iso`, `.cso` (CISO v1) or DVD `.chd`, and `EBOOT.PBP`
/// eboots (PSN games, homebrew). Recognised by content, never by extension: `.iso`
/// / `.chd` / `.pbp` are shared with PlayStation (`PSP GAME` volume or `PSP_GAME`
/// directory; a PBP that is not a PS1 classic). Out: `.elf` / `.prx` (tests only),
/// `.zso` / `.dax` / `.jso` (the core cannot read them), archives, `.m3u`. A UMD is
/// at most 1.8 GB; 8 GiB leaves room for large PSN eboots.
pub const PSP: SystemSpec = SystemSpec {
    id: "psp",
    display_name: "Sony PlayStation Portable",
    short_label: "PSP",
    media: Media::Disc,
    extensions: &["iso", "cso", "chd", "pbp"],
    probe: ContentProbe::Disc(DiscSignature::Psp),
    header_rule: HeaderRule::None,
    min_bytes: 16,
    max_bytes: 8 * 1024 * 1024 * 1024,
    default_core_id: "ppsspp",
    dat_file: "Sony - PlayStation Portable.dat",
    metadat_file: "Sony - PlayStation Portable.dat",
    thumbnails_repo: "Sony_-_PlayStation_Portable",
    thumbnails_folder: "Sony - PlayStation Portable",
    // PPSSPP RetroPad map: B/A/X/Y = Cross/Circle/Triangle/Square, L/R = shoulders,
    // left stick = the analog nub. L2/R2 stay rewind / fast-forward hotkeys.
    pad_actions: &[
        "b",
        "a",
        "y",
        "x",
        "select",
        "start",
        "up",
        "down",
        "left",
        "right",
        "l",
        "r",
        "lstick_up",
        "lstick_down",
        "lstick_left",
        "lstick_right",
    ],
    pad_labels: &[
        ("b", "✕"),
        ("a", "○"),
        ("y", "□"),
        ("x", "△"),
        ("l", "L"),
        ("r", "R"),
        ("lstick_up", "Stick Up"),
        ("lstick_down", "Stick Down"),
        ("lstick_left", "Stick Left"),
        ("lstick_right", "Stick Right"),
    ],
    analog_sticks: AnalogSticks::Both,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(41),
};

#[cfg(test)]
#[path = "psp_system_spec_tests.rs"]
mod tests;
