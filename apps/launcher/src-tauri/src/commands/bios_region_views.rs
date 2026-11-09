//! Settings → BIOS region rows: what each region boots. `KnownFiles` cores answer
//! with `FirmwareReport::choice_for` (first valid file of the region, else any, else
//! HLE); `Ps2Romdir` cores with the shared picker `ps2_bios_pick::choose`, so the row
//! names exactly the file the runner's `pcsx2_bios` answer boots for a disc of that
//! region (region → newest → name), and no HLE row exists.

use serde::Serialize;
use system_catalog::{
    bios_regions, ps2_bios_pick, BiosChoice, CoreSpec, FirmwareReport, FirmwareRule, ANY_REGION,
};

/// What a region boots: a real BIOS file, or `None` = built-in HLE BIOS (or, for a
/// core without HLE, no BIOS at all).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiosRegionView {
    pub region: String,
    /// Row title ("JP", or "All regions" for a BIOS that boots every region).
    pub label: String,
    pub file_name: Option<String>,
}

/// One row per region the core distinguishes.
pub fn region_views(core: &CoreSpec, report: &FirmwareReport) -> Vec<BiosRegionView> {
    bios_regions(core)
        .iter()
        .map(|region| BiosRegionView {
            region: (*region).to_owned(),
            label: match *region {
                ANY_REGION => "All regions".to_owned(),
                code => code.to_owned(),
            },
            file_name: booted_file(core, report, region),
        })
        .collect()
}

fn booted_file(core: &CoreSpec, report: &FirmwareReport, region: &str) -> Option<String> {
    match core.firmware_rule {
        FirmwareRule::Ps2Romdir { .. } => {
            ps2_bios_pick::choose(report, Some(region)).map(|f| f.file_name.clone())
        }
        FirmwareRule::KnownFiles => match report.choice_for(region) {
            BiosChoice::Real { file_name } => Some(file_name),
            BiosChoice::Hle => None,
        },
    }
}

/// Whether the core boots without any BIOS file (built-in HLE BIOS). PS2 cores do
/// not: Settings says so instead of "Built-in HLE BIOS".
pub fn has_hle_fallback(core: &CoreSpec) -> bool {
    matches!(core.firmware_rule, FirmwareRule::KnownFiles)
}
