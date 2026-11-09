//! Settings → BIOS: per core with BIOS files (PCSX ReARMed, Flycast, PCEE2), its
//! BIOS folder, the BIOS files found there and what each region boots; add a picked
//! file; open the folder. A BIOS-switch core's file counts only with a known md5
//! (the runner checks the same before letting the core boot it); a PS2 core's file
//! counts with a ROMDIR `ROMVER` entry and comes with its sidecars. Nostyra never
//! ships or downloads BIOS files.

use std::path::{Path, PathBuf};

use serde::Serialize;
use system_catalog::{
    firmware_dir, scan_firmware, systems_for_core, CoreSpec, FirmwareRule, CORES,
};
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::command_error::{CommandError, CommandResult};
use crate::commands::bios_import::{identify, import_name, max_bios_bytes};
use crate::commands::bios_region_views::{has_hle_fallback, region_views, BiosRegionView};
use crate::commands::bios_sidecar_import::copy_sidecars;
use crate::commands::blocking::run_blocking;
use crate::commands::ps2_bios_import::{pick_extensions, stored_stem, warning_for};

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
    /// Decoded version label ("v1.60 · USA · 2002-02-07"); PS2 dumps only.
    pub label: Option<String>,
    /// Usable but discouraged (PS2 v1.00): shown with a warning.
    pub warning: Option<String>,
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
    /// The core boots without a BIOS file (built-in HLE BIOS); false for PS2.
    pub hle_fallback: bool,
    /// File-picker extensions for "Add BIOS file…" (empty: any file).
    pub pick_extensions: Vec<String>,
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
/// dump; PS2: stem kept, sidecars copied beside it); returns the stored name. An
/// existing different file with that name is never overwritten.
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
            match std::fs::read(&target) {
                Ok(existing) if existing == bytes => {}
                _ => {
                    return Err(CommandError::invalid(format!(
                        "A different file named {name} is already in the BIOS folder. \
                         Remove or rename it first."
                    )))
                }
            }
        } else {
            std::fs::create_dir_all(&dir).map_err(storage)?;
            let partial = dir.join(format!("{name}.part"));
            std::fs::write(&partial, &bytes).map_err(storage)?;
            std::fs::rename(&partial, &target).map_err(storage)?;
            tracing::info!(file = %target.display(), "BIOS file added");
        }
        if let FirmwareRule::Ps2Romdir { .. } = core.firmware_rule {
            let copied = copy_sidecars(&src, &dir, &stored_stem(&picked)).map_err(storage)?;
            if !copied.is_empty() {
                tracing::info!(sidecars = ?copied, "BIOS sidecars copied");
            }
        }
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

/// Cores Settings → BIOS lists: a firmware list, or a list-less rule (PS2).
fn bios_cores() -> impl Iterator<Item = &'static CoreSpec> {
    CORES.into_iter().filter(|core| {
        !core.firmware.is_empty() || matches!(core.firmware_rule, FirmwareRule::Ps2Romdir { .. })
    })
}

fn system_dir(app: &AppHandle) -> PathBuf {
    ipc_protocol::system_dir(&app.state::<AppState>().paths.data_root)
}

pub(crate) fn core_view(core: &'static CoreSpec, dir: &Path) -> CoreBiosView {
    let folder = firmware_dir(dir, core);
    let mut report = scan_firmware(dir, core);
    let mut files = Vec::new();
    for file in &mut report.files {
        // md5 identifies known dumps; a PS2 file was already read and decoded.
        let md5_known = match core.firmware_rule {
            FirmwareRule::KnownFiles => std::fs::read(folder.join(&file.file_name))
                .ok()
                .and_then(|bytes| identify(&bytes, core)),
            FirmwareRule::Ps2Romdir { .. } => None,
        };
        if core.bios_switch.is_some() {
            file.valid = md5_known.is_some(); // the runner boots only a known dump
        }
        let known = file.known.or(md5_known);
        files.push(BiosFileView {
            file_name: file.file_name.clone(),
            model: known.map(|k| k.model.to_owned()),
            region: known.map_or(file.region, |k| k.region).to_owned(),
            valid: file.valid,
            label: file.label.clone(),
            warning: warning_for(file),
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
        hle_fallback: has_hle_fallback(core),
        pick_extensions: pick_extensions(core),
    }
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
