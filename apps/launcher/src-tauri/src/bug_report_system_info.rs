//! `system-info.txt` of a bug report: Nostyra version, OS version, data-folder kind
//! and the installed cores with their versions. No paths or account data (the logs
//! beside it already show what a maintainer needs). The GPU is not listed: the
//! launcher never opens one; the runner logs its adapter in each game session log.

use std::path::Path;
use std::process::Command;

use crate::app_state::AppState;

/// Facts written to `system-info.txt` (gathered separately so the text is testable).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SystemInfoFacts {
    pub os_version: Option<String>,
    /// "portable", "default" or "custom".
    pub data_folder_kind: &'static str,
    /// Installed cores: display name and version (`None` = placed by hand).
    pub cores: Vec<(String, Option<String>)>,
}

/// Collects the facts from the running launcher (spawns `ver` on Windows).
pub fn gather_system_info(state: &AppState) -> SystemInfoFacts {
    let db = state.db();
    let cores = system_catalog::CORES
        .into_iter()
        .filter(|core| core_manager::installed_core(&state.paths.cores_dir, core.id).is_some())
        .map(|core| {
            let record = storage_db::get_installed_core(&db, core.id).ok().flatten();
            (
                core.display_name.to_string(),
                record.and_then(|r| r.version),
            )
        })
        .collect();
    SystemInfoFacts {
        os_version: os_version(),
        data_folder_kind: data_folder_kind(&state.paths.data_root),
        cores,
    }
}

/// The text of `system-info.txt`.
pub fn system_info_text(facts: &SystemInfoFacts) -> String {
    let mut text = format!(
        "Nostyra version: {}\nOS: {}\nOS version: {}\nArchitecture: {}\nData folder: {}\nCreated: {}\n\nInstalled cores:\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        facts.os_version.as_deref().unwrap_or("unknown"),
        std::env::consts::ARCH,
        facts.data_folder_kind,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z"),
    );
    if facts.cores.is_empty() {
        text.push_str("- none\n");
    }
    for (name, version) in &facts.cores {
        let version = version.as_deref().unwrap_or("version unknown");
        text.push_str(&format!("- {name}: {version}\n"));
    }
    text
}

/// Portable copy, the per-user default, or a folder chosen in Settings.
fn data_folder_kind(data_root: &Path) -> &'static str {
    if ipc_protocol::default_data_root().as_deref() != Some(data_root) {
        "custom"
    } else if ipc_protocol::portable_dir().is_some() {
        "portable"
    } else {
        "default"
    }
}

/// `Microsoft Windows [Version 10.0.26200.…]` from `cmd /C ver` (no console window);
/// `uname -sr` elsewhere. `None` when the command fails.
fn os_version() -> Option<String> {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = Command::new("cmd");
        command.args(["/C", "ver"]).creation_flags(CREATE_NO_WINDOW);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("uname");
        command.arg("-sr");
        command
    };
    let output = command.output().ok().filter(|out| out.status.success())?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_lists_version_os_data_folder_and_cores() {
        let facts = SystemInfoFacts {
            os_version: Some("Microsoft Windows [Version 10.0.26200.1]".into()),
            data_folder_kind: "portable",
            cores: vec![
                ("Snes9x".into(), Some("1.63".into())),
                ("Mesen".into(), None),
            ],
        };
        let text = system_info_text(&facts);
        assert!(text.contains(&format!("Nostyra version: {}\n", env!("CARGO_PKG_VERSION"))));
        assert!(text.contains("OS version: Microsoft Windows [Version 10.0.26200.1]\n"));
        assert!(text.contains("Data folder: portable\n"));
        assert!(
            text.contains("- Snes9x: 1.63\n- Mesen: version unknown\n"),
            "{text}"
        );
        let empty = system_info_text(&SystemInfoFacts::default());
        assert!(empty.contains("OS version: unknown") && empty.contains("- none"));
    }

    #[test]
    fn os_version_is_known_on_this_pc() {
        assert!(os_version().is_some_and(|v| !v.is_empty()));
    }
}
