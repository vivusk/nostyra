//! PS2 BIOS sidecars: beside a picked dump, PCSX2 installs keep `<stem>.ROM1` /
//! `.ROM2` / `.EROM` (DVD player / extra ROM modules, loaded by stem) and the
//! console's `.NVM` / `.MEC` (NVRAM, MechaCon; the core creates defaults when they
//! are missing). Import copies the ones present as `<stored stem>.<ext lower>` into
//! the BIOS folder; `.DIFF` / `.INF` (dumper metadata) are never copied, and an
//! existing target is left alone (the core may already have written its own
//! `.nvm` / `.mec`).

use std::path::Path;

/// Sidecar extensions copied (case-insensitive match on the source).
pub const SIDECAR_EXTS: [&str; 5] = ["rom1", "rom2", "erom", "nvm", "mec"];

/// Copies `picked`'s sidecars into `dir` under `stored_stem`; returns the stored
/// names, in [`SIDECAR_EXTS`] order. A missing source folder copies nothing.
pub fn copy_sidecars(picked: &Path, dir: &Path, stored_stem: &str) -> std::io::Result<Vec<String>> {
    let Some(stem) = picked.file_stem().and_then(|s| s.to_str()) else {
        return Ok(Vec::new());
    };
    let Some(source_dir) = picked.parent() else {
        return Ok(Vec::new());
    };
    let mut copied = Vec::new();
    for ext in SIDECAR_EXTS {
        let Some(source) = find_sidecar(source_dir, stem, ext)? else {
            continue;
        };
        let name = format!("{stored_stem}.{ext}");
        let target = dir.join(&name);
        if target.exists() {
            continue; // never over the core's own NVRAM / an earlier import
        }
        let partial = dir.join(format!("{name}.part"));
        std::fs::copy(&source, &partial)?;
        std::fs::rename(&partial, &target)?;
        copied.push(name);
    }
    Ok(copied)
}

/// `<stem>.<ext>` in `source_dir`, matched without regard to case (Windows users
/// keep `.ROM1`, Linux dumps `.rom1`).
fn find_sidecar(
    source_dir: &Path,
    stem: &str,
    ext: &str,
) -> std::io::Result<Option<std::path::PathBuf>> {
    let wanted = format!("{stem}.{ext}");
    let entries = match std::fs::read_dir(source_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.eq_ignore_ascii_case(&wanted) && entry.file_type()?.is_file() {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_sidecars_are_copied_lower_cased_under_the_stored_stem() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("SCPH-77001_BIOS_V15_USA_220_(NTSC)");
        std::fs::create_dir_all(&src).unwrap();
        for (name, bytes) in [
            ("SCPH-77001_BIOS_V15_USA_220.ROM0", &[1u8; 32][..]),
            ("SCPH-77001_BIOS_V15_USA_220.ROM1", &[2u8; 32]),
            ("SCPH-77001_BIOS_V15_USA_220.DIFF", &[3u8; 32]),
            ("SCPH-77001_BIOS_V15_USA_220.NVM", &[4u8; 1024]),
            ("SCPH-77001_BIOS_V15_USA_220.MEC", &[5u8; 4]),
            ("SCPH-77001_BIOS_V15_USA_220.INF", b"meta"),
        ] {
            std::fs::write(src.join(name), bytes).unwrap();
        }
        let dir = temp.path().join("system/pcsx2/bios");
        std::fs::create_dir_all(&dir).unwrap();
        let picked = src.join("SCPH-77001_BIOS_V15_USA_220.ROM0");
        let copied = copy_sidecars(&picked, &dir, "scph-77001_bios_v15_usa_220").unwrap();
        assert_eq!(
            copied,
            [
                "scph-77001_bios_v15_usa_220.rom1",
                "scph-77001_bios_v15_usa_220.nvm",
                "scph-77001_bios_v15_usa_220.mec",
            ]
        );
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "scph-77001_bios_v15_usa_220.mec",
                "scph-77001_bios_v15_usa_220.nvm",
                "scph-77001_bios_v15_usa_220.rom1",
            ],
            "no .DIFF / .INF / .part"
        );
        assert_eq!(
            std::fs::read(dir.join("scph-77001_bios_v15_usa_220.nvm")).unwrap(),
            vec![4u8; 1024]
        );
        // Second import: existing targets stay (the core's own NVRAM is never clobbered).
        std::fs::write(dir.join("scph-77001_bios_v15_usa_220.nvm"), [9u8; 1024]).unwrap();
        let again = copy_sidecars(&picked, &dir, "scph-77001_bios_v15_usa_220").unwrap();
        assert!(again.is_empty(), "{again:?}");
        assert_eq!(
            std::fs::read(dir.join("scph-77001_bios_v15_usa_220.nvm")).unwrap(),
            vec![9u8; 1024]
        );
    }

    #[test]
    fn a_lone_dump_copies_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let picked = temp.path().join("scph39001.bin");
        std::fs::write(&picked, [0u8; 16]).unwrap();
        let dir = temp.path().join("bios");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(copy_sidecars(&picked, &dir, "scph39001")
            .unwrap()
            .is_empty());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        let gone = temp.path().join("missing/x.bin");
        assert!(copy_sidecars(&gone, &dir, "x").unwrap().is_empty());
    }
}
