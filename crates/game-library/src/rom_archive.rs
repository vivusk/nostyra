//! Single-ROM ZIPs shared by library scanning and the runner. Archive names are
//! identities; member names/bytes decide the system, title and canonical hashes.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path};

use system_catalog::{classify_with_head, ClassifyError, SystemSpec, ALL_SYSTEMS, PROBE_BYTES};
use zip::ZipArchive;

/// A validated cartridge member. `bytes` are the original, unmodified ROM dump.
#[derive(Debug)]
pub struct ArchivedRom {
    pub name: String,
    pub system: &'static SystemSpec,
    pub bytes: Vec<u8>,
}

pub fn is_rom_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

/// Reads only the candidate's head (rescan discovery never inflates whole ROMs).
pub(crate) fn probe_rom_archive(path: &Path) -> io::Result<&'static SystemSpec> {
    read_archive(path, false).map(|rom| rom.system)
}

/// Reads and validates exactly one supported ROM, enforcing declared and actual
/// sizes. Ancillary documents are ignored; other ROMs/archives are rejected.
pub fn read_rom_archive(path: &Path) -> io::Result<ArchivedRom> {
    read_archive(path, true)
}

fn supported(system: &SystemSpec) -> bool {
    matches!(
        system.id,
        "nes" | "snes" | "gba" | "genesis" | "sms" | "gg" | "sg1000"
    )
}

fn invalid(reason: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason.into())
}

fn read_archive(path: &Path, whole: bool) -> io::Result<ArchivedRom> {
    let mut archive = ZipArchive::new(File::open(path)?)?;
    let systems: Vec<_> = ALL_SYSTEMS
        .into_iter()
        .filter(|spec| supported(spec))
        .collect();
    let mut selected = None;
    for index in 0..archive.len() {
        // Metadata inspection does not require a decoder for unrelated documents.
        let entry = archive.by_index_raw(index)?;
        let name = entry.name()?.into_owned();
        let member = Path::new(&name);
        if entry.is_dir()
            || member
                .components()
                .any(|part| part.as_os_str() == "__MACOSX")
            || member
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with("._"))
        {
            continue;
        }
        if entry.enclosed_name().is_none()
            || name.contains(['\\', ':'])
            || member
                .components()
                .any(|part| matches!(part, Component::ParentDir))
            || !entry.is_file()
        {
            return Err(invalid(format!("unsafe ZIP member: {name}")));
        }
        let ext = member.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("zip") {
            return Err(invalid("nested ZIP archives are not supported"));
        }
        if !ALL_SYSTEMS.iter().any(|spec| spec.has_extension(ext)) {
            continue;
        }
        let candidates: Vec<_> = systems
            .iter()
            .copied()
            .filter(|s| s.has_extension(ext))
            .collect();
        if candidates.is_empty() {
            return Err(invalid(format!(
                "ZIP ROM system is not supported: {name} (NES, SNES, Sega cartridges and GBA only)"
            )));
        }
        let size = entry.size();
        if !candidates.iter().any(|spec| spec.accepts_size(size)) {
            // A short README.md is a document, not an invalid Genesis dump.
            if ext.eq_ignore_ascii_case("md") {
                continue;
            }
            return Err(invalid(format!(
                "ROM in ZIP is outside the system size limits: {name}"
            )));
        }
        drop(entry);
        let mut bytes = Vec::new();
        archive
            .by_index(index)?
            .take(PROBE_BYTES as u64)
            .read_to_end(&mut bytes)?;
        let system = match classify_with_head(
            &systems,
            ext,
            size,
            Some(&bytes),
            member,
            &disc_image::DiscImageProbe,
        ) {
            Ok(system) => system,
            Err(ClassifyError::NotRom(_)) => continue,
            Err(err) => return Err(invalid(format!("{name}: {err}"))),
        };
        if selected.is_some() {
            return Err(invalid(
                "ZIP must contain exactly one ROM; multiple ROMs were found",
            ));
        }
        selected = Some((index, name, system, size, bytes));
    }
    let (index, name, system, size, mut bytes) = selected.ok_or_else(|| {
        invalid("ZIP contains no supported ROM (NES, SNES, Sega cartridges or GBA)")
    })?;
    if whole {
        bytes.clear();
        archive
            .by_index(index)?
            .take(system.max_bytes + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != size || bytes.len() as u64 > system.max_bytes {
            return Err(invalid("ROM in ZIP has an invalid decompressed size"));
        }
    }
    Ok(ArchivedRom {
        name,
        system,
        bytes,
    })
}
