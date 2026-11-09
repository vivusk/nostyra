//! Web edition (EmulatorJS 4.2.3) cores: which EJS core plays a system in the
//! browser ([`SystemSpec::web_core`](crate::SystemSpec)), where that core keeps its
//! SRAM inside EmulatorJS's file system, and which desktop core is the same
//! libretro core (for the shared `config/core/<id>.toml` layer).

use crate::core_specs::{core_for_system, CoreSpec};
use crate::system_specs::system;

/// An EmulatorJS core as the web edition uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebCore {
    /// `EJS_core` value (EmulatorJS core name, e.g. `pcsx_rearmed`).
    pub ejs_core: &'static str,
    /// RetroArch's per-core save folder in EmulatorJS's file system: SRAM lives at
    /// `/data/saves/<save_dir>/<content stem>.srm` (phase 00 spike, D4).
    pub save_dir: &'static str,
}

/// NES: FCEUmm (the desktop plays NES on Mesen, which EmulatorJS lacks).
pub const WEB_FCEUMM: WebCore = WebCore {
    ejs_core: "fceumm",
    save_dir: "FCEUmm",
};
/// SNES: Snes9x.
pub const WEB_SNES9X: WebCore = WebCore {
    ejs_core: "snes9x",
    save_dir: "Snes9x",
};
/// GB / GBC / GBA: mGBA (forced; EmulatorJS's GB default is Gambatte).
pub const WEB_MGBA: WebCore = WebCore {
    ejs_core: "mgba",
    save_dir: "mGBA",
};
/// Mega Drive / Master System / Game Gear / SG-1000: Genesis Plus GX (forced for SMS).
pub const WEB_GENESIS_PLUS_GX: WebCore = WebCore {
    ejs_core: "genesis_plus_gx",
    save_dir: "Genesis Plus GX",
};
/// PlayStation: PCSX ReARMed.
pub const WEB_PCSX_REARMED: WebCore = WebCore {
    ejs_core: "pcsx_rearmed",
    save_dir: "PCSX-ReARMed",
};

/// Atari 2600: Stella 2014 (the old 3.9 core; the desktop plays Stella 8, so states
/// are never shared). No SRAM on any Atari web core (`save_dir` unused).
pub const WEB_STELLA2014: WebCore = WebCore {
    ejs_core: "stella2014",
    save_dir: "Stella 2014",
};
/// Atari 7800: ProSystem.
pub const WEB_PROSYSTEM: WebCore = WebCore {
    ejs_core: "prosystem",
    save_dir: "ProSystem",
};
/// Atari Lynx: Handy (its EEPROM file stays in EmulatorJS's file system, not synced;
/// the desktop's fixed `handy_refresh_rate=60` is not applied on the web).
pub const WEB_HANDY: WebCore = WebCore {
    ejs_core: "handy",
    save_dir: "Handy",
};

/// Every EmulatorJS core the web edition ships.
pub const WEB_CORES: [WebCore; 8] = [
    WEB_FCEUMM,
    WEB_SNES9X,
    WEB_MGBA,
    WEB_GENESIS_PLUS_GX,
    WEB_PCSX_REARMED,
    WEB_STELLA2014,
    WEB_PROSYSTEM,
    WEB_HANDY,
];

/// EmulatorJS core → desktop core id, for cores that are the same libretro core.
const SAME_LIBRETRO_CORE: [(&str, &str); 6] = [
    ("snes9x", "snes9x"),
    ("mgba", "mgba"),
    ("genesis_plus_gx", "genesis_plus_gx"),
    ("pcsx_rearmed", "pcsx-rearmed"),
    ("prosystem", "prosystem"),
    ("handy", "handy"),
];

/// EmulatorJS cores with no desktop counterpart (no shared core layer).
pub const WEB_ONLY_CORES: [&str; 2] = ["fceumm", "stella2014"];

/// The desktop core that is the same libretro core as the system's web core: the web
/// edition then reads/writes its `config/core/<id>.toml` layer too. `None` when the
/// system is not on the web or its web core differs from the desktop one (NES).
pub fn web_shared_core(system_id: &str) -> Option<&'static CoreSpec> {
    let web = system(system_id)?.web_core?;
    let desktop = core_for_system(system_id)?;
    SAME_LIBRETRO_CORE
        .iter()
        .any(|&(ejs, id)| ejs == web.ejs_core && id == desktop.id)
        .then_some(desktop)
}

#[cfg(test)]
#[path = "web_core_specs_tests.rs"]
mod tests;
