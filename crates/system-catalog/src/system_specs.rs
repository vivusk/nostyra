//! The systems Nostyra knows, as data. Adding a system = one `SystemSpec` entry
//! (+ its DAT files and a `platforms` row migration); scanner, launcher and runner
//! code read the fields and need no change.

use std::path::Path;

use crate::disc_probe::Media;
use crate::disc_system_specs::PSX;
use crate::dreamcast_system_spec::DREAMCAST;
use crate::gamecube_wii_system_specs::{GC, WII};
use crate::handheld_system_specs::{GB, GBA, GBC};
use crate::nds_system_spec::NDS;
use crate::nintendo_64_system_spec::NINTENDO_64;
use crate::psp_system_spec::PSP;
use crate::rom_header_rules::{ContentProbe, HeaderRule};
use crate::sega_system_specs::{GENESIS, GG, SG1000, SMS};
pub use crate::snes_nes_system_specs::{NES, SNES};
use crate::touch_screen_rule::TouchScreenRule;

/// Everything Nostyra needs to know about one system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemSpec {
    /// storage-db `platforms.id`; also the saves/config folder name.
    pub id: &'static str,
    /// UI name ("Super Nintendo").
    pub display_name: &'static str,
    /// Badge / filter label ("SNES").
    pub short_label: &'static str,
    /// Cartridge (one file per game) or disc (multi-file games, serial identity).
    pub media: Media,
    /// Lower-case extensions without the dot.
    pub extensions: &'static [&'static str],
    pub probe: ContentProbe,
    pub header_rule: HeaderRule,
    /// Whole-file size limits (header included); files outside are skipped by the
    /// scanner and refused by the runner, so Play never fails on a listed game.
    pub min_bytes: u64,
    pub max_bytes: u64,
    /// Core used to play this system (an id in `CORES`).
    pub default_core_id: &'static str,
    /// No-Intro DAT file name in the vendored libretro-database folder.
    pub dat_file: &'static str,
    /// File name inside each `metadat/<attribute>/` folder.
    pub metadat_file: &'static str,
    /// libretro-thumbnails GitHub repository (spaces → `_`).
    pub thumbnails_repo: &'static str,
    /// Playlist folder on thumbnails.libretro.com.
    pub thumbnails_folder: &'static str,
    /// RetroPad actions this system's pad has, in UI order (Controls editor subset).
    pub pad_actions: &'static [&'static str],
    /// The system's own button name per RetroPad action (`("y", "A")` on the Mega
    /// Drive), shown beside the RetroPad name since bindings are shared across
    /// systems. Empty: the RetroPad names are the system's names.
    pub pad_labels: &'static [(&'static str, &'static str)],
    /// Which SDL sticks are reported raw as `RETRO_DEVICE_ANALOG`; a stick reported
    /// raw is never mapped onto the D-pad.
    pub analog_sticks: AnalogSticks,
    /// The pad has analog triggers (L2 / R2 pressure, `RETRO_DEVICE_ANALOG` index 2):
    /// SDL trigger axes are reported raw instead of as digital buttons (Dreamcast).
    pub analog_triggers: bool,
    /// Touch screen or aiming pointer fed by the mouse (host pointer, port 0); the
    /// rule says where it sits in the output frame (DS bottom screen, Wii Remote IR).
    /// `None`: no pointer input.
    pub touch_screen: Option<TouchScreenRule>,
    /// RetroAchievements console id (rcheevos `RC_CONSOLE_*`, what `rc_hash` and
    /// `rc_libretro` take). `None`: no achievements on this system (GameCube / Wii:
    /// out of scope, rcheevos has no frontend support for their core).
    pub ra_console_id: Option<u8>,
}

/// Which pad sticks a system reads as analog axes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnalogSticks {
    /// No analog input: the left stick acts as the D-pad (SNES).
    #[default]
    None,
    /// Both sticks raw (Nintendo 64, Dreamcast, PSP).
    Both,
    /// Right stick raw, left stick still the D-pad (Nintendo DS stick cursor).
    RightOnly,
}

impl SystemSpec {
    /// Case-insensitive extension check.
    pub fn matches_extension(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| self.has_extension(ext))
    }

    /// Case-insensitive check of an extension without the dot.
    pub fn has_extension(&self, ext: &str) -> bool {
        self.extensions
            .iter()
            .any(|allowed| ext.eq_ignore_ascii_case(allowed))
    }

    /// Whole-file size within the system limits.
    pub fn accepts_size(&self, file_len: u64) -> bool {
        (self.min_bytes..=self.max_bytes).contains(&file_len)
    }
}

/// Every system, in classification priority order (first passing candidate wins).
/// Cartridges first, then discs: a `.bin` checks the cheap Mega Drive magic before
/// the PlayStation disc probe, and a `.cue` / `.chd` / `.m3u` shared by PlayStation
/// and Dreamcast tries the PlayStation probe first (it fails fast on a GD-ROM);
/// GameCube / Wii come last (an `.iso` fails the PSX / PSP probes after one read).
pub const ALL_SYSTEMS: [&SystemSpec; 16] = [
    &SNES,
    &NES,
    &GB,
    &GBC,
    &GBA,
    &NINTENDO_64,
    &NDS,
    &GENESIS,
    &SMS,
    &GG,
    &SG1000,
    &PSX,
    &DREAMCAST,
    &PSP,
    &GC,
    &WII,
];

/// Every system in UI order (sidebar, Library system filter): makers by popularity
/// (Nintendo, Sony, Sega); per maker, home consoles then handhelds, each by release.
/// Display only — classification keeps [`ALL_SYSTEMS`] order.
pub const DISPLAY_ORDER: [&SystemSpec; 16] = [
    &NES,
    &SNES,
    &NINTENDO_64,
    &GC,
    &WII,
    &GB,
    &GBC,
    &GBA,
    &NDS,
    &PSX,
    &PSP,
    &SG1000,
    &SMS,
    &GENESIS,
    &DREAMCAST,
    &GG,
];

/// System by `platforms.id`.
pub fn system(id: &str) -> Option<&'static SystemSpec> {
    ALL_SYSTEMS.into_iter().find(|spec| spec.id == id)
}

#[cfg(test)]
#[path = "system_specs_tests.rs"]
mod tests;
