//! Disc systems (`Media::Disc`): games are CD images, possibly several files per
//! game (`.cue` + track files, `.m3u` multi-disc playlists). Content decides, so
//! `.bin` / `.iso` / `.cue` stay shareable with future disc systems.

use crate::disc_probe::{DiscSignature, Media};
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::system_specs::{AnalogSticks, SystemSpec};

/// Sony PlayStation: Redump `.cue` + `.bin` tracks, bare `.bin` / `.iso`, `.chd`,
/// PS1 classic `.pbp` eboots and `.m3u` multi-disc playlists. Sheets are tiny
/// (16 B floor); 4 GiB covers a multi-disc `.pbp`. The real check is the probe.
pub const PSX: SystemSpec = SystemSpec {
    id: "psx",
    display_name: "PlayStation",
    short_label: "PS1",
    media: Media::Disc,
    extensions: &["cue", "bin", "iso", "chd", "pbp", "m3u"],
    probe: ContentProbe::Disc(DiscSignature::PlayStation),
    header_rule: HeaderRule::None,
    min_bytes: 16,
    max_bytes: 4 * 1024 * 1024 * 1024,
    default_core_id: "pcsx-rearmed",
    dat_file: "Sony - PlayStation.dat",
    metadat_file: "Sony - PlayStation.dat",
    thumbnails_repo: "Sony_-_PlayStation",
    thumbnails_folder: "Sony - PlayStation",
    // RetroPad names: Cross = b, Circle = a, Square = y, Triangle = x.
    pad_actions: &[
        "b", "a", "y", "x", "select", "start", "up", "down", "left", "right", "l", "r", "l2", "r2",
    ],
    pad_labels: &[],
    analog_sticks: AnalogSticks::None,
    analog_triggers: false,
    touch_screen: None,
    ra_console_id: Some(12),
};
