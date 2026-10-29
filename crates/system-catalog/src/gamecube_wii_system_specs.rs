//! Nintendo GameCube and Wii, both played by Dolphin (OpenGL hardware rendering).
//! A disc is recognised by its header magic (GC `C2339F3D` @0x1C, Wii `5D1C9EA3`
//! @0x18), never by its extension: `.iso` is shared with PlayStation and PSP, the
//! other containers with each other. Identity = the 6-char game ID (no whole-file
//! hash; see `game-library` `disc_game_identity`).

use crate::disc_probe::{DiscSignature, Media};
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};
use crate::touch_screen_rule::TouchScreenRule;

/// Nintendo GameCube: plain `.iso` / `.gcm`, the compressed `.ciso` / `.gcz` /
/// `.wia` / `.rvz` containers, homebrew `.dol` / `.elf` (no Wii-only code) and
/// `.m3u` multi-disc playlists. 16 B floor (sheets; the probe checks real
/// headers); 2 GiB covers a full mini-DVD image (1,459,978,240 B). Out: `.tgc`,
/// NKit, `.wbfs` (Wii only), archives.
pub const GC: SystemSpec = SystemSpec {
    id: "gc",
    display_name: "Nintendo GameCube",
    short_label: "GC",
    media: Media::Disc,
    extensions: &[
        "iso", "gcm", "ciso", "gcz", "rvz", "wia", "dol", "elf", "m3u",
    ],
    probe: ContentProbe::Disc(DiscSignature::GameCube),
    header_rule: HeaderRule::None,
    min_bytes: 16,
    max_bytes: 2 * 1024 * 1024 * 1024,
    default_core_id: "dolphin-emu",
    dat_file: "Nintendo - GameCube.dat",
    metadat_file: "Nintendo - GameCube.dat",
    thumbnails_repo: "Nintendo_-_GameCube",
    thumbnails_folder: "Nintendo - GameCube",
    // Dolphin's GameCube pad map: RetroPad A/B/X/Y = GC A/B/X/Y, R = Z, L2 / R2 =
    // the analog L / R triggers, L3 / R3 = their digital "light press" click. No
    // Select. The pad's LT / RT hold hotkeys yield to L2 / R2 (`input_conflicts`).
    pad_actions: &[
        "a",
        "b",
        "x",
        "y",
        "start",
        "up",
        "down",
        "left",
        "right",
        "r",
        "l2",
        "r2",
        "l3",
        "r3",
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
        ("a", "A"),
        ("b", "B"),
        ("x", "X"),
        ("y", "Y"),
        ("r", "Z"),
        ("l2", "L"),
        ("r2", "R"),
        ("l3", "L (light press)"),
        ("r3", "R (light press)"),
        ("lstick_up", "Stick Up"),
        ("lstick_down", "Stick Down"),
        ("lstick_left", "Stick Left"),
        ("lstick_right", "Stick Right"),
        ("rstick_up", "C-Stick Up"),
        ("rstick_down", "C-Stick Down"),
        ("rstick_left", "C-Stick Left"),
        ("rstick_right", "C-Stick Right"),
    ],
    // C-stick = right stick; L / R analog = RetroPad L2 / R2 pressure.
    analog_sticks: AnalogSticks::Both,
    analog_triggers: true,
    touch_screen: None,
    ra_console_id: None,
};

/// Nintendo Wii: plain `.iso`, `.ciso` / `.gcz` / `.wia` / `.rvz` / `.wbfs`
/// containers and homebrew `.dol` / `.elf` with Wii-only code. 8 GiB covers a
/// dual-layer disc (8,511,160,320 B). No `.m3u` (multi-disc Wii games do not
/// exist), no `.wad` (WiiWare / VC are out of scope). The Wii Remote's IR pointer
/// is aimed with the mouse (port 0 only; mouse buttons map to nothing).
pub const WII: SystemSpec = SystemSpec {
    id: "wii",
    display_name: "Nintendo Wii",
    short_label: "Wii",
    media: Media::Disc,
    extensions: &["iso", "ciso", "gcz", "rvz", "wia", "wbfs", "dol", "elf"],
    probe: ContentProbe::Disc(DiscSignature::Wii),
    header_rule: HeaderRule::None,
    min_bytes: 16,
    max_bytes: 8 * 1024 * 1024 * 1024,
    default_core_id: "dolphin-emu",
    dat_file: "Nintendo - Wii.dat",
    metadat_file: "Nintendo - Wii.dat",
    thumbnails_repo: "Nintendo_-_Wii",
    thumbnails_folder: "Nintendo - Wii",
    // Wii Remote + Nunchuk (fixed `dolphin_wiimote_default_device`), Dolphin's map
    // in IR mode 2 (spike `SET_INPUT_DESCRIPTORS` dump). The shakes are digital,
    // so the pad's LT / RT hold hotkeys yield to L2 / R2 (`input_conflicts`).
    pad_actions: &[
        "a",
        "b",
        "x",
        "y",
        "select",
        "start",
        "up",
        "down",
        "left",
        "right",
        "l",
        "r",
        "l2",
        "r2",
        "r3",
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
        ("a", "A"),
        ("b", "B"),
        ("x", "Nunchuk C"),
        ("y", "Nunchuk Z"),
        ("start", "1"),
        ("select", "2"),
        ("l", "\u{2212}"),
        ("r", "+"),
        ("l2", "Shake Nunchuk"),
        ("r2", "Shake Remote"),
        ("r3", "Home"),
        ("lstick_up", "Nunchuk Stick Up"),
        ("lstick_down", "Nunchuk Stick Down"),
        ("lstick_left", "Nunchuk Stick Left"),
        ("lstick_right", "Nunchuk Stick Right"),
        ("rstick_up", "Tilt Remote Up"),
        ("rstick_down", "Tilt Remote Down"),
        ("rstick_left", "Tilt Remote Left"),
        ("rstick_right", "Tilt Remote Right"),
    ],
    // Left stick = Nunchuk stick; right stick = Wii Remote tilt (IR mode 2).
    analog_sticks: AnalogSticks::Both,
    analog_triggers: false,
    // IR pointer = the mouse over the whole picture (`MousePointer` in the runner).
    touch_screen: Some(TouchScreenRule::WiiRemoteIr),
    ra_console_id: None,
};

#[cfg(test)]
#[path = "gamecube_wii_system_spec_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gamecube_wii_pad_spec_tests.rs"]
mod pad_tests;
