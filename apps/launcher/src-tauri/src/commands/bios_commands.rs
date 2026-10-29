//! Settings → BIOS: per core with BIOS files (PCSX ReARMed, Flycast), its BIOS
//! folder, the BIOS files found there and what each region boots; add a picked
//! file; open the folder. A BIOS-switch core's file counts only with a known md5
//! (the runner checks the same before letting the core boot it). Nostyra never ships
//! or downloads BIOS files.

use std::path::{Path, PathBuf};

use serde::Serialize;
use system_catalog::{
    bios_regions, firmware_dir, scan_firmware, systems_for_core, BiosChoice, CoreSpec,
    FirmwareReport, ANY_REGION, CORES,
};
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::command_error::{CommandError, CommandResult};
use crate::commands::bios_import::{identify, import_name, max_bios_bytes};
use crate::commands::blocking::run_blocking;

/// One file in the system folder that looks like a BIOS.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiosFileView {
    pub file_name: String,
    /// Console model ("SCPH-1001") when the name or the md5 is known.
    pub model: Option<String>,
    pub region: String,
    /// Fails the size / signature check: the core ignores it.
    pub valid: bool,
}

/// What a region boots: a real BIOS file, or `None` = built-in HLE BIOS.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiosRegionView {
    pub region: String,
    /// Row title ("JP", or "All regions" for a BIOS that boots every region).
    pub label: String,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreBiosView {
    pub core_id: String,
    pub core_name: String,
    pub system_names: Vec<String>,
    pub folder: String,
    pub files: Vec<BiosFileView>,
    pub regions: Vec<BiosRegionView>,
}

#[tauri::command]
pub async fn get_bios_status(app: AppHandle) -> CommandResult<Vec<CoreBiosView>> {
    run_blocking(move || {
        let dir = system_dir(&app);
        Ok(bios_cores().map(|core| core_view(core, &dir)).collect())
    })
    .await
}

/// Copies a picked BIOS image into the system folder (canonical name for a known
/// dump); returns the stored name. An existing different file with that name is
/// never overwritten.
#[tauri::command]
pub async fn add_bios_file(app: AppHandle, core_id: String, path: String) -> CommandResult<String> {
    run_blocking(move || {
        let core = bios_cores()
            .find(|c| c.id == core_id)
            .ok_or_else(|| CommandError::invalid("This core uses no BIOS files."))?;
        let src = PathBuf::from(&path);
        let picked = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = read_capped(&src, max_bios_bytes(core))?;
        let name = import_name(&picked, &bytes, core).map_err(CommandError::invalid)?;
        let dir = firmware_dir(&system_dir(&app), core);
        let target = dir.join(&name);
        if target.is_file() {
            return match std::fs::read(&target) {
                Ok(existing) if existing == bytes => Ok(name),
                _ => Err(CommandError::invalid(format!(
                    "A different file named {name} is already in the BIOS folder. \
                     Remove or rename it first."
                ))),
            };
        }
        std::fs::create_dir_all(&dir).map_err(storage)?;
        let partial = dir.join(format!("{name}.part"));
        std::fs::write(&partial, &bytes).map_err(storage)?;
        std::fs::rename(&partial, &target).map_err(storage)?;
        tracing::info!(file = %target.display(), "BIOS file added");
        Ok(name)
    })
    .await
}

#[tauri::command]
pub async fn open_system_folder(app: AppHandle) -> CommandResult<()> {
    run_blocking(move || {
        let dir = system_dir(&app);
        std::fs::create_dir_all(&dir).map_err(storage)?;
        tauri_plugin_opener::open_path(&dir, None::<&str>)
            .map_err(|err| CommandError::new("storage", format!("Cannot open the folder: {err}")))
    })
    .await
}

fn bios_cores() -> impl Iterator<Item = &'static CoreSpec> {
    CORES.into_iter().filter(|core| !core.firmware.is_empty())
}

fn system_dir(app: &AppHandle) -> PathBuf {
    ipc_protocol::system_dir(&app.state::<AppState>().paths.data_root)
}

pub(crate) fn core_view(core: &'static CoreSpec, dir: &Path) -> CoreBiosView {
    let folder = firmware_dir(dir, core);
    let mut report = scan_firmware(dir, core);
    let mut files = Vec::new();
    for file in &mut report.files {
        let md5_known = std::fs::read(folder.join(&file.file_name))
            .ok()
            .and_then(|bytes| identify(&bytes, core));
        if core.bios_switch.is_some() {
            file.valid = md5_known.is_some(); // the runner boots only a known dump
        }
        let known = file.known.or(md5_known);
        files.push(BiosFileView {
            file_name: file.file_name.clone(),
            model: known.map(|k| k.model.to_owned()),
            region: known.map_or(file.region, |k| k.region).to_owned(),
            valid: file.valid,
        });
    }
    CoreBiosView {
        core_id: core.id.to_owned(),
        core_name: core.display_name.to_owned(),
        system_names: systems_for_core(core.id)
            .iter()
            .map(|s| s.display_name.to_owned())
            .collect(),
        folder: folder.display().to_string(),
        files,
        regions: region_views(core, &report),
    }
}

fn region_views(core: &CoreSpec, report: &FirmwareReport) -> Vec<BiosRegionView> {
    bios_regions(core)
        .iter()
        .map(|region| BiosRegionView {
            region: (*region).to_owned(),
            label: match *region {
                ANY_REGION => "All regions".to_owned(),
                code => code.to_owned(),
            },
            file_name: match report.choice_for(region) {
                BiosChoice::Real { file_name } => Some(file_name),
                BiosChoice::Hle => None,
            },
        })
        .collect()
}

/// Reads at most one byte past `max` (anything bigger is refused anyway).
fn read_capped(path: &Path, max: u64) -> CommandResult<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(max + 1).read_to_end(&mut bytes))
        .map_err(|err| CommandError::invalid(format!("Cannot read {}: {err}", path.display())))?;
    Ok(bytes)
}

fn storage(err: std::io::Error) -> CommandError {
    CommandError::new("storage", format!("Cannot write to the BIOS folder: {err}"))
}

#[cfg(test)]
#[path = "bios_commands_tests.rs"]
mod tests;
