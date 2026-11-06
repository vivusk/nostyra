//! The scan's verdict on one `.zip`: shared by the desktop folder scan and the web
//! scan, so the skip-versus-problem policy lives in one place.

use std::path::Path;

use system_catalog::SystemSpec;

use crate::rom_archive::{is_not_single_rom, probe_rom_archive};

/// What a scan does with a `.zip` among the `allowed` systems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipScan {
    /// A single ROM of this allowed system (read lazily; the head only).
    Rom(&'static SystemSpec),
    /// Not a single-ROM ZIP of an allowed system (arcade set, several ROMs, another
    /// system, nested ZIP): skipped quietly like any non-ROM file.
    Skip,
    /// Damaged, unsafe or undecodable archive: a scan problem with this reason.
    Problem(String),
}

/// Classifies the `.zip` at `path` without inflating the whole ROM.
pub fn scan_zip(path: &Path, allowed: &[&'static SystemSpec]) -> ZipScan {
    match probe_rom_archive(path) {
        Ok(system) if allowed.contains(&system) => ZipScan::Rom(system),
        Ok(_) => ZipScan::Skip,
        Err(err) if is_not_single_rom(&err) => ZipScan::Skip,
        Err(err) => ZipScan::Problem(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use system_catalog::system;
    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::*;

    fn zip_at(dir: &Path, name: &str, entries: &[(&str, Vec<u8>)]) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut writer = ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (member, bytes) in entries {
            writer
                .start_file(*member, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    #[test]
    fn single_rom_is_ok_other_cases_skip_or_problem() {
        let dir = tempfile::tempdir().unwrap();
        let rom = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms/nostyra-test.sfc"),
        )
        .unwrap();
        let snes = system("snes").unwrap();
        let nes = system("nes").unwrap();

        let one = zip_at(dir.path(), "one.zip", &[("a.sfc", rom.clone())]);
        assert_eq!(scan_zip(&one, &[snes]), ZipScan::Rom(snes));
        assert_eq!(scan_zip(&one, &[nes]), ZipScan::Skip);

        let two = zip_at(
            dir.path(),
            "two.zip",
            &[("a.sfc", rom.clone()), ("b.sfc", rom)],
        );
        assert_eq!(scan_zip(&two, &[snes]), ZipScan::Skip);

        let docs = zip_at(dir.path(), "docs.zip", &[("readme.txt", b"hi".to_vec())]);
        assert_eq!(scan_zip(&docs, &[snes]), ZipScan::Skip);

        let broken = dir.path().join("broken.zip");
        std::fs::write(&broken, b"not a zip at all").unwrap();
        assert!(matches!(scan_zip(&broken, &[snes]), ZipScan::Problem(_)));
    }
}
