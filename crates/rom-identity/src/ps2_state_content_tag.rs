//! PlayStation 2 arm of `state_content_tag`: the content id is the serial only
//! (`ps2_disc_identity`), so every dump and revision of one serial shares a save
//! folder, and PCEE2 loads a state from another disc silently (M18 spike step 7:
//! "ELF changed", `Ok`). The tag names the `BOOT2` executable and the CRC32 of its
//! first 64 KiB (`SLUS_203.12;1:1A2B3C4D`), read through the same header-only disc
//! reader every container shares (`.iso`, `.bin/.cue`, `.chd`, `.cso`, `.zso`).
//! An `.m3u` is tagged `m3u:` + its first disc's tag, as on GameCube (playlist
//! states load only in playlist sessions). No `SYSTEM.CNF`, no executable or an
//! unreadable image: `None` (no check; the runner logs it).

use std::path::Path;

use disc_image::iso9660_root::{read_extent, PrimaryVolume};
use disc_image::{boot2_exe, is_m3u_ext, open_data_track, playlist_entries, system_cnf, DiscError};

/// Bytes of the boot executable hashed into the tag (one header read; the whole ELF
/// is megabytes).
const EXE_HASH_BYTES: usize = 64 * 1024;

/// The tag of the PS2 game `rom` (see the module doc).
pub(super) fn tag(rom: &Path) -> Option<String> {
    let ext = rom.extension().and_then(|e| e.to_str()).unwrap_or("");
    let playlist = is_m3u_ext(ext);
    let disc = match playlist {
        true => playlist_entries(rom).ok()?.into_iter().next()?,
        false => rom.to_path_buf(),
    };
    let tag = match disc_tag(&disc) {
        Ok(Some(tag)) => tag,
        Ok(None) => {
            tracing::warn!(disc = %disc.display(), "no BOOT2 executable: state content tag skipped");
            return None;
        }
        Err(err) => {
            tracing::warn!(disc = %disc.display(), %err, "state content tag skipped");
            return None;
        }
    };
    Some(match playlist {
        true => format!("m3u:{tag}"),
        false => tag,
    })
}

/// `"<BOOT2 exe>:<CRC32 of its first 64 KiB>"`, `None` without a `BOOT2` line or
/// when the named executable is not on the disc.
fn disc_tag(disc: &Path) -> Result<Option<String>, DiscError> {
    let mut reader = open_data_track(disc)?;
    let reader = reader.as_mut();
    let pvd = PrimaryVolume::read(reader)?;
    let Some(exe) = system_cnf(reader, &pvd)?.as_deref().and_then(boot2_exe) else {
        return Ok(None);
    };
    // `DEMO\BOOT.ELF;1` → ["DEMO", "BOOT.ELF"] (the `;1` version is the record's).
    let components: Vec<&str> = exe
        .split(['\\', '/'])
        .filter_map(|part| part.split(';').next())
        .filter(|part| !part.is_empty())
        .collect();
    let Some(extent) = pvd.find_path(reader, &components)? else {
        return Ok(None);
    };
    let head = read_extent(reader, extent, EXE_HASH_BYTES)?;
    Ok(Some(format!("{exe}:{:08X}", crc32fast::hash(&head))))
}

#[cfg(test)]
#[path = "ps2_state_content_tag_tests.rs"]
mod tests;
