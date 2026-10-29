//! Settings → BIOS → "Add BIOS file…" decisions (pure). PlayStation: a 512 KiB
//! image with the BIOS signature; a known dump (md5 in the core's firmware list) is
//! stored under its canonical lower-case name (cores look names up case-sensitively
//! on Linux/macOS), anything else under its own name lower-cased. Cores with a BIOS
//! switch (Dreamcast): only a known dump (size + md5), stored under its canonical
//! name, since the core would boot any file with that name.

use md5::{Digest, Md5};
use system_catalog::{is_valid_bios, systems_for_core, CoreSpec, FirmwareFile};

/// Lower-case hex md5 of `bytes`.
pub fn md5_hex(bytes: &[u8]) -> String {
    Md5::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The core's firmware entry with this image's md5 (and size).
pub fn identify<'a>(bytes: &[u8], core: &'a CoreSpec) -> Option<&'a FirmwareFile> {
    let md5 = md5_hex(bytes);
    core.firmware
        .iter()
        .find(|f| f.md5 == md5 && f.size == bytes.len() as u64)
}

/// The largest BIOS file the core lists (read cap for a picked file).
pub fn max_bios_bytes(core: &CoreSpec) -> u64 {
    core.firmware.iter().map(|f| f.size).max().unwrap_or(0)
}

/// File name to store a picked image under, or why it is refused.
pub fn import_name(picked_name: &str, bytes: &[u8], core: &CoreSpec) -> Result<String, String> {
    let system = systems_for_core(core.id)
        .iter()
        .map(|s| s.display_name)
        .collect::<Vec<_>>()
        .join(" / ");
    let not_bios = format!("{picked_name} is not a {system} BIOS");
    if core.bios_switch.is_some() {
        return match identify(bytes, core) {
            Some(known) => Ok(known.name.to_owned()),
            None => {
                let wanted = core.firmware.first().map_or_else(String::new, |f| {
                    format!("{} ({})", f.name, size_text(f.size))
                });
                Err(format!(
                    "{not_bios}: expected {wanted} with a known checksum."
                ))
            }
        };
    }
    if !core.firmware.iter().any(|f| f.size == bytes.len() as u64) {
        let size = size_text(max_bios_bytes(core));
        return Err(format!("{not_bios}: a BIOS file is exactly {size}."));
    }
    if !is_valid_bios(bytes) {
        return Err(format!("{not_bios} (its signature is missing)."));
    }
    Ok(match identify(bytes, core) {
        Some(known) => known.name.to_owned(),
        None => picked_name.to_lowercase(),
    })
}

/// `524288` → "512 KiB", `2097152` → "2 MiB".
fn size_text(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 && b % (1 << 20) == 0 => format!("{} MiB", b >> 20),
        b => format!("{} KiB", b >> 10),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use system_catalog::{BIOS_BYTES, FLYCAST, PCSX_REARMED};

    fn image() -> Vec<u8> {
        let mut bytes = vec![0u8; BIOS_BYTES as usize];
        bytes[..5].copy_from_slice(&[0x13, 0x00, 0x08, 0x3C, 0x3F]);
        bytes
    }

    #[test]
    fn unknown_dump_keeps_its_name_lower_cased() {
        assert_eq!(
            import_name("My BIOS.BIN", &image(), &PCSX_REARMED).as_deref(),
            Ok("my bios.bin")
        );
        assert_eq!(identify(&image(), &PCSX_REARMED), None);
    }

    #[test]
    fn invalid_files_are_refused() {
        let short = import_name("x.bin", &[0u8; 1024], &PCSX_REARMED).unwrap_err();
        assert!(short.contains("512 KiB"), "{short}");
        let zeros = vec![0u8; BIOS_BYTES as usize];
        let unsigned = import_name("x.bin", &zeros, &PCSX_REARMED).unwrap_err();
        assert!(unsigned.contains("signature"), "{unsigned}");
    }

    #[test]
    fn dreamcast_takes_only_a_known_dump_under_its_canonical_name() {
        let fake = b"nostyra fake dreamcast bios";
        let file = FirmwareFile {
            md5: Box::leak(md5_hex(fake).into_boxed_str()),
            size: fake.len() as u64,
            ..FLYCAST.firmware[0]
        };
        let core = CoreSpec {
            firmware: Box::leak(Box::new([file])),
            ..FLYCAST
        };
        assert_eq!(
            import_name("My DC.bin", fake, &core).as_deref(),
            Ok("dc_boot.bin")
        );
        let unknown = import_name("x.bin", &vec![0u8; 2 * 1024 * 1024], &FLYCAST).unwrap_err();
        assert_eq!(
            unknown,
            "x.bin is not a Sega Dreamcast BIOS: expected dc_boot.bin (2 MiB) with a known checksum."
        );
        assert_eq!(max_bios_bytes(&FLYCAST), 2 * 1024 * 1024);
        assert_eq!(max_bios_bytes(&PCSX_REARMED), BIOS_BYTES);
    }

    #[test]
    fn md5_is_lower_case_hex() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    /// Dev-only: the developer's own SCPH-1001 dump (read-only) maps to its
    /// canonical name. `cargo test -p nostyra-launcher scph1001 -- --ignored`
    #[test]
    #[ignore = "needs the developer's own BIOS dump"]
    fn developer_scph1001_gets_its_canonical_name() {
        let bytes = std::fs::read(r"E:\ePSXe v2.0.5\bios\SCPH1001.bin").unwrap_or_default();
        assert_eq!(
            import_name("SCPH1001.bin", &bytes, &PCSX_REARMED).as_deref(),
            Ok("scph1001.bin")
        );
        assert_eq!(
            identify(&bytes, &PCSX_REARMED).map(|f| f.model),
            Some("SCPH-1001")
        );
    }
}
