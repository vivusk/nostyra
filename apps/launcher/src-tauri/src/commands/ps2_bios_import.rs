//! Settings → BIOS → "Add BIOS file…" for `FirmwareRule::Ps2Romdir` cores (pure):
//! any 4–8 MiB image with a ROMDIR `ROMVER` entry is accepted whatever its
//! extension (`.BIN`, `.ROM0`) and stored as `<picked stem, lower-case>.bin` — the
//! stem is kept so the user's `.ROM1` / `.NVM` / `.MEC` sidecars (copied beside it
//! with the same stem, `bios_sidecar_import`) stay attached the way PCSX2 loads them.

use system_catalog::{parse_ps2_bios, BiosFileStatus, CoreSpec, FirmwareRule, Ps2BiosVersion};

/// Extension every imported PS2 dump is stored under.
const STORED_EXT: &str = "bin";

/// Settings warning for a v1.00 PS2 dump (never a refusal).
const V100_WARNING: &str =
    "v1.00 (SCPH-10000) has known memory-card bugs; prefer a later BIOS version";

/// "Add BIOS file…" picker extensions: PS2 dumps come as `.BIN` / `.ROM0` (any
/// case); other cores keep the open picker (empty).
pub fn pick_extensions(core: &CoreSpec) -> Vec<String> {
    match core.firmware_rule {
        FirmwareRule::Ps2Romdir { .. } => ["bin", "rom0", "BIN", "ROM0"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        FirmwareRule::KnownFiles => Vec::new(),
    }
}

/// Settings warning for a usable but discouraged dump (PS2 v1.00).
pub fn warning_for(file: &BiosFileStatus) -> Option<String> {
    file.ps2_version
        .as_ref()
        .is_some_and(Ps2BiosVersion::is_discouraged)
        .then(|| V100_WARNING.to_owned())
}

/// `SCPH-39001_BIOS_V7_USA_160.BIN` → `scph-39001_bios_v7_usa_160`: the picked name
/// without its last extension, lower-cased, path separators dropped.
pub fn stored_stem(picked_name: &str) -> String {
    let name = picked_name
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(picked_name);
    let stem = match name.rsplit_once('.') {
        Some((stem, _ext)) if !stem.is_empty() => stem,
        _ => name,
    };
    stem.to_lowercase()
}

/// File name to store a picked PS2 image under, or why it is refused (with the
/// decoded version for the success notice).
pub fn import_name(
    picked_name: &str,
    bytes: &[u8],
    system: &str,
) -> Result<(String, Ps2BiosVersion), String> {
    match parse_ps2_bios(bytes) {
        Some(version) => Ok((
            format!("{}.{STORED_EXT}", stored_stem(picked_name)),
            version,
        )),
        None => Err(format!(
            "{picked_name} is not a {system} BIOS (no ROMDIR/ROMVER entry; .ROM1 / .DIFF / .NVM \
             are not the BIOS)."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use system_catalog::ps2_bios_test_image::{ps2_bios_image, ps2_rom1_like_image};

    #[test]
    fn stored_name_is_the_lower_cased_stem_plus_bin() {
        assert_eq!(
            stored_stem("SCPH-39001_BIOS_V7_USA_160.BIN"),
            "scph-39001_bios_v7_usa_160"
        );
        assert_eq!(
            stored_stem("SCPH-77001_BIOS_V15_USA_220.ROM0"),
            "scph-77001_bios_v15_usa_220"
        );
        assert_eq!(stored_stem("noext"), "noext");
        assert_eq!(stored_stem(".hidden"), ".hidden", "no empty stem");
        assert_eq!(
            stored_stem("dir\\sub/Name.Rom0"),
            "name",
            "separators dropped"
        );
    }

    #[test]
    fn valid_dumps_are_renamed_and_others_refused_with_the_sidecar_hint() {
        let image = ps2_bios_image("0220AC20060210", 0x2780);
        let (name, version) =
            import_name("SCPH-77001_BIOS_V15_USA_220.ROM0", &image, "PlayStation 2")
                .expect("valid");
        assert_eq!(name, "scph-77001_bios_v15_usa_220.bin");
        assert_eq!(version.label(), "v2.20 · USA · 2006-02-10");
        let err = import_name(
            "SCPH-77001_BIOS_V15_USA_220.ROM1",
            &ps2_rom1_like_image(),
            "PlayStation 2",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "SCPH-77001_BIOS_V15_USA_220.ROM1 is not a PlayStation 2 BIOS (no ROMDIR/ROMVER entry; \
             .ROM1 / .DIFF / .NVM are not the BIOS)."
        );
        assert!(import_name("x.bin", &[0u8; 1024], "PlayStation 2").is_err());
    }
}
