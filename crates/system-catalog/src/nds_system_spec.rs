//! Nintendo DS, played by melonDS DS (software renderer, touch screen via pointer).

use crate::disc_probe::Media;
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};
use crate::touch_screen_rule::TouchScreenRule;

/// Nintendo DS: decrypted cartridge dumps `.nds` (No-Intro "Decrypted"), `.dsi`
/// (DS + DSi enhanced) and `.ids` (iQue DS), recognised by the header CRC; DSi-only
/// games are refused with their own reason. 4 KiB (melonDS DS refuses smaller
/// files) .. 512 MiB (the largest DAT entry). Whole-file hash; large files are
/// streamed. Out: archives, `.srl`, `.bcn`, `.bin`.
pub const NDS: SystemSpec = SystemSpec {
    id: "nds",
    display_name: "Nintendo DS",
    short_label: "NDS",
    media: Media::Cartridge,
    extensions: &["nds", "dsi", "ids"],
    probe: ContentProbe::NdsHeader,
    header_rule: HeaderRule::None,
    min_bytes: 0x1000,
    max_bytes: 512 * 1024 * 1024,
    default_core_id: "melonds_ds",
    dat_file: "Nintendo - Nintendo DS.dat",
    metadat_file: "Nintendo - Nintendo DS.dat",
    thumbnails_repo: "Nintendo_-_Nintendo_DS",
    thumbnails_folder: "Nintendo - Nintendo DS",
    // melonDS DS RetroPad map = the DS buttons, plus R2 = touch at the stick cursor
    // (right stick, `touch_mode=auto`), L3 = blow into the microphone (hold), R3 =
    // swap the screen layout. The left stick stays the D-pad; the pad's RT
    // fast-forward hold yields to R2 (`input_conflicts`), LT rewind stays.
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
        "r2",
        "l3",
        "r3",
        "rstick_up",
        "rstick_down",
        "rstick_left",
        "rstick_right",
    ],
    pad_labels: &[
        ("r2", "Touch (stick cursor)"),
        ("l3", "Microphone (blow)"),
        ("r3", "Swap screens"),
        ("rstick_up", "Cursor Up"),
        ("rstick_down", "Cursor Down"),
        ("rstick_left", "Cursor Left"),
        ("rstick_right", "Cursor Right"),
    ],
    analog_sticks: AnalogSticks::RightOnly,
    analog_triggers: false,
    touch_screen: Some(TouchScreenRule::DsBottomScreen),
    ra_console_id: Some(18),
};

#[cfg(test)]
#[path = "nds_system_spec_tests.rs"]
mod tests;
