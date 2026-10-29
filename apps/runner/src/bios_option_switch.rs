//! Real or HLE BIOS for cores whose BIOS option the runner answers
//! (`CoreSpec.bios_switch`, Flycast): such a core boots any file with the BIOS name,
//! and a wrong one ends in a fatal emulator error (M11 phase-00 spike). So the real
//! BIOS is chosen only when a known file sits where the core looks
//! (`<system>/<subdir>/<name>`) with the registry's size and md5; otherwise the core
//! is told to use its built-in HLE BIOS.

use std::path::Path;

use md5::{Digest, Md5};
use system_catalog::{CoreSpec, FirmwareFile, SystemSpec};

/// What the runner tells a BIOS-switch core, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BiosSwitchChoice {
    /// A known BIOS file (`subdir/name`).
    Real(String),
    /// No BIOS file: the core's HLE BIOS.
    Hle,
    /// A file with the BIOS name that is not a known dump (`subdir/name`, its md5).
    Unknown(String, String),
}

impl BiosSwitchChoice {
    /// Log text, e.g. "real BIOS dc/dc_boot.bin".
    pub fn describe(&self) -> String {
        match self {
            Self::Real(file) => format!("real BIOS {file}"),
            Self::Hle => "HLE BIOS (built-in)".to_owned(),
            Self::Unknown(file, md5) => {
                format!("{file} is not a known BIOS (md5 {md5}); using the HLE BIOS (built-in)")
            }
        }
    }
}

/// The choice for `core` (`None` for cores without a BIOS switch).
pub fn choose(core: &CoreSpec, system_dir: &Path) -> Option<BiosSwitchChoice> {
    core.bios_switch?;
    let mut choice = BiosSwitchChoice::Hle;
    for file in core.firmware {
        let shown = shown_path(file);
        let Ok(bytes) = std::fs::read(system_dir.join(file.subdir).join(file.name)) else {
            continue;
        };
        let md5 = hex(&Md5::digest(&bytes));
        if bytes.len() as u64 == file.size && md5 == file.md5 {
            return Some(BiosSwitchChoice::Real(shown));
        }
        choice = BiosSwitchChoice::Unknown(shown, md5);
    }
    Some(choice)
}

/// At launch: logs the BIOS choice for `system`'s core and returns the option to
/// answer (`None` for cores without a BIOS switch).
pub fn launch_option(system: &SystemSpec, system_dir: &Path) -> Option<(String, String)> {
    let core = system_catalog::core_for_system(system.id)?;
    let choice = choose(core, system_dir)?;
    tracing::info!("{} BIOS: {}", system.display_name, choice.describe());
    option_for(core, &choice)
}

/// The `(key, value)` core option for `choice`.
pub fn option_for(core: &CoreSpec, choice: &BiosSwitchChoice) -> Option<(String, String)> {
    let switch = core.bios_switch?;
    let value = match choice {
        BiosSwitchChoice::Real(_) => switch.real,
        BiosSwitchChoice::Hle | BiosSwitchChoice::Unknown(..) => switch.hle,
    };
    Some((switch.key.to_owned(), value.to_owned()))
}

fn shown_path(file: &FirmwareFile) -> String {
    match file.subdir {
        "" => file.name.to_owned(),
        dir => format!("{dir}/{}", file.name),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "bios_option_switch_tests.rs"]
mod tests;
