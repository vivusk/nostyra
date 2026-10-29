//! Sega Dreamcast, played by Flycast (OpenGL hardware rendering).

use crate::disc_probe::{DiscSignature, Media};
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};

/// Sega Dreamcast: GD-ROM dumps as TOSEC `.gdi` + tracks or Redump `.cue` + tracks,
/// `.chd`, DiscJuggler `.cdi` (MIL-CD / self-boot) and `.m3u` multi-disc playlists.
/// A disc is recognised by IP.BIN (`SEGA SEGAKATANA`), never by its extension:
/// `.cue` / `.chd` / `.m3u` are shared with PlayStation. `.bin` is not listed (Flycast
/// would route it to its arcade loader), nor are arcade archives or `.elf`. Sheets
/// are tiny (16 B floor); a `.cdi` / `.chd` stays well below 4 GiB.
pub const DREAMCAST: SystemSpec = SystemSpec {
    id: "dreamcast",
    display_name: "Sega Dreamcast",
    short_label: "DC",
    media: Media::Disc,
    extensions: &["gdi", "cdi", "chd", "cue", "m3u"],
    probe: ContentProbe::Disc(DiscSignature::Dreamcast),
    header_rule: HeaderRule::None,
    min_bytes: 16,
    max_bytes: 4 * 1024 * 1024 * 1024,
    default_core_id: "flycast",
    dat_file: "Sega - Dreamcast.dat",
    metadat_file: "Sega - Dreamcast.dat",
    thumbnails_repo: "Sega_-_Dreamcast",
    thumbnails_folder: "Sega - Dreamcast",
    // Flycast RetroPad map: B/A/Y/X = Dreamcast A/B/X/Y, L2/R2 = the analog
    // triggers, left stick = the analog stick.
    pad_actions: &[
        "b",
        "a",
        "y",
        "x",
        "start",
        "up",
        "down",
        "left",
        "right",
        "l2",
        "r2",
        "lstick_up",
        "lstick_down",
        "lstick_left",
        "lstick_right",
    ],
    pad_labels: &[
        ("b", "A"),
        ("a", "B"),
        ("y", "X"),
        ("x", "Y"),
        ("l2", "LT"),
        ("r2", "RT"),
        ("lstick_up", "Stick Up"),
        ("lstick_down", "Stick Down"),
        ("lstick_left", "Stick Left"),
        ("lstick_right", "Stick Right"),
    ],
    analog_sticks: AnalogSticks::Both,
    analog_triggers: true,
    touch_screen: None,
    ra_console_id: Some(40),
};

#[cfg(test)]
#[path = "dreamcast_system_spec_tests.rs"]
mod tests;
