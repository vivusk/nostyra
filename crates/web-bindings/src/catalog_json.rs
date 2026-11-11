//! The system registry as the web launcher needs it: the desktop `SystemView` fields
//! (same camelCase names as `get_systems`) plus what only the web uses — media, the
//! EmulatorJS core and its save folder, the shared core config layer, BIOS files.

use serde::Serialize;
use system_catalog::{
    core_for_system, netplay_eligibility, netplay_max_players, web_shared_core, Media,
    NetplaySupport, SystemSpec, ALL_SYSTEMS, DISPLAY_ORDER,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSystemView {
    pub id: &'static str,
    pub display_name: &'static str,
    pub short_label: &'static str,
    pub extensions: &'static [&'static str],
    pub pad_actions: &'static [&'static str],
    pub pad_labels: Vec<PadLabel>,
    pub core_id: &'static str,
    /// Display name of the desktop core (`"PCSX ReARMed"`; Settings → BIOS).
    pub core_name: &'static str,
    /// `"cartridge"` / `"disc"`.
    pub media: &'static str,
    /// EmulatorJS core; `None` = not playable in the browser.
    pub web_core: Option<WebCoreView>,
    /// `config/core/<id>.toml` the web reads and writes too (same libretro core as
    /// the desktop); `None` = no core layer on the web (NES: FCEUmm ≠ Mesen).
    pub core_layer_id: Option<&'static str>,
    /// BIOS files of the desktop core (shown in Settings → BIOS, fed to EmulatorJS).
    pub firmware: Vec<FirmwareView>,
    /// Bundled No-Intro / Redump DAT and metadat file names (libretro-database).
    pub dat_file: &'static str,
    pub metadat_file: &'static str,
    /// libretro-thumbnails repository of the system's artwork.
    pub thumbnails_repo: &'static str,
    /// Position in the Library system filter (`DISPLAY_ORDER`).
    pub display_rank: usize,
    /// Play online support of the DESKTOP core (`"yes"` / `"experimental"` / `"no"`,
    /// the launcher's registry pre-check). The browser has no netplay: it shows
    /// Play online with a "desktop app only" notice for these systems.
    pub desktop_netplay: &'static str,
    /// Largest desktop netplay room of the system (players).
    pub desktop_netplay_players: u8,
}

#[derive(Debug, Serialize)]
pub struct PadLabel {
    pub action: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebCoreView {
    pub ejs_core: &'static str,
    pub save_dir: &'static str,
}

#[derive(Debug, Serialize)]
pub struct FirmwareView {
    pub name: &'static str,
    /// Console model ("SCPH-5501"), shown for a file stored under this name.
    pub model: &'static str,
    pub md5: &'static str,
    pub region: &'static str,
    pub size: u64,
    pub subdir: &'static str,
}

/// Every registry system, registry order.
pub fn web_systems() -> Vec<WebSystemView> {
    ALL_SYSTEMS.iter().map(|spec| view(spec)).collect()
}

fn view(spec: &'static SystemSpec) -> WebSystemView {
    WebSystemView {
        id: spec.id,
        display_name: spec.display_name,
        short_label: spec.short_label,
        extensions: spec.extensions,
        pad_actions: spec.pad_actions,
        pad_labels: spec
            .pad_labels
            .iter()
            .map(|&(action, label)| PadLabel { action, label })
            .collect(),
        core_id: spec.default_core_id,
        core_name: core_for_system(spec.id).map_or("", |core| core.display_name),
        media: match spec.media {
            Media::Cartridge => "cartridge",
            Media::Disc => "disc",
        },
        web_core: spec.web_core.map(|core| WebCoreView {
            ejs_core: core.ejs_core,
            save_dir: core.save_dir,
        }),
        core_layer_id: web_shared_core(spec.id).map(|core| core.id),
        dat_file: spec.dat_file,
        metadat_file: spec.metadat_file,
        thumbnails_repo: spec.thumbnails_repo,
        display_rank: DISPLAY_ORDER
            .iter()
            .position(|shown| shown.id == spec.id)
            .unwrap_or(usize::MAX),
        desktop_netplay: desktop_netplay(spec.id),
        desktop_netplay_players: core_for_system(spec.id)
            .map_or(0, |core| netplay_max_players(core, spec.id)),
        firmware: core_for_system(spec.id)
            .map(|core| core.firmware)
            .unwrap_or_default()
            .iter()
            .map(|f| FirmwareView {
                name: f.name,
                model: f.model,
                md5: f.md5,
                region: f.region,
                size: f.size,
                subdir: f.subdir,
            })
            .collect(),
    }
}

/// The desktop launcher's registry-only netplay pre-check, as a JSON tag.
fn desktop_netplay(system_id: &str) -> &'static str {
    let support = core_for_system(system_id)
        .and_then(|core| netplay_eligibility(core, core.savestates, None).ok());
    match support {
        Some(NetplaySupport::Yes) => "yes",
        Some(NetplaySupport::Experimental) => "experimental",
        Some(NetplaySupport::No) | None => "no",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_fields_follow_the_registry() {
        let json = serde_json::to_value(web_systems()).unwrap();
        let psx = json
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == "psx")
            .unwrap();
        assert_eq!(psx["media"], "disc");
        assert_eq!(psx["webCore"]["ejsCore"], "pcsx_rearmed");
        assert_eq!(psx["webCore"]["saveDir"], "PCSX-ReARMed");
        assert_eq!(psx["coreLayerId"], "pcsx-rearmed");
        assert!(!psx["firmware"].as_array().unwrap().is_empty());
        let nes = json
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == "nes")
            .unwrap();
        assert_eq!(nes["webCore"]["ejsCore"], "fceumm");
        assert!(nes["coreLayerId"].is_null());
        assert!(nes.get("displayName").is_some() && nes.get("padActions").is_some());
        assert_eq!(nes["displayRank"], 0, "NES leads the Library filter");
        assert_eq!(nes["desktopNetplay"], "yes");
        assert_eq!(psx["desktopNetplay"], "experimental");
        let by_id = |id: &str| {
            json.as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == id)
                .unwrap()
        };
        assert_eq!(by_id("lynx")["desktopNetplay"], "no");
        assert_eq!(
            by_id("n64")["desktopNetplay"],
            "no",
            "GL cores never offer netplay"
        );
        assert!(by_id("snes")["desktopNetplayPlayers"].as_u64().unwrap() >= 2);
        assert_eq!(psx["datFile"], "Sony - PlayStation.dat");
        assert_eq!(psx["thumbnailsRepo"], "Sony_-_PlayStation");
    }
}
