//! Save-state content tag (`StateSidecar::content_tag`): which disc / revision a state
//! was made from, for systems whose games share one save folder across discs and
//! revisions (GameCube / Wii: the content id is the game ID only, see
//! `gamecube_wii_identity`). Dolphin loads any blob, so a disc-2 or Rev 1 state must
//! never reach a disc-1 / Rev 0 session: the runner refuses a different tag and the
//! launcher marks it `state-incompatible` (`ipc_protocol::state_incompatibility`).
//! Shared by both so they compute the same tag from the same file.
//!
//! - GameCube / Wii disc image: `"<ID6>:<disc>:<rev>"` (`GALE01:0:1`) from its header
//!   (one ≤ 64 KiB read; ISO, RVZ, … copies of one disc share it).
//! - `.m3u`: `"m3u:<ID6>"` of its first disc (playlist states load only in playlist
//!   sessions; disc swaps inside stay the core's business).
//! - `.dol` / `.elf`, every other system, or an unreadable header: `None` (no check).

use std::path::Path;

use disc_image::{is_executable_ext, is_m3u_ext, playlist_entries, read_nintendo_disc_header};
use system_catalog::{ContentProbe, DiscSignature, SystemSpec};

/// The content tag of the game `rom` launched as `system` (see the module doc).
pub fn state_content_tag(system: &SystemSpec, rom: &Path) -> Option<String> {
    let ContentProbe::Disc(DiscSignature::GameCube | DiscSignature::Wii) = system.probe else {
        return None;
    };
    let ext = rom.extension().and_then(|e| e.to_str()).unwrap_or("");
    if is_executable_ext(ext) {
        return None;
    }
    let playlist = is_m3u_ext(ext);
    let disc = match playlist {
        true => playlist_entries(rom).ok()?.into_iter().next()?,
        false => rom.to_path_buf(),
    };
    let header = match read_nintendo_disc_header(&disc) {
        Ok(Some(header)) => header,
        Ok(None) | Err(_) => {
            tracing::warn!(disc = %disc.display(), "no disc header: state content tag skipped");
            return None;
        }
    };
    let id = header.game_id_text();
    Some(match playlist {
        true => format!("m3u:{id}"),
        false => format!("{id}:{}:{}", header.disc, header.revision),
    })
}

#[cfg(test)]
mod tests {
    use disc_image::test_nintendo_images::{disc_header, dol, plain, wia, BLR};
    use disc_image::NintendoDiscKind::{GameCube, Wii};
    use system_catalog::{GC, SNES, WII};

    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn discs_and_revisions_get_their_own_tag_containers_do_not() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let disc1 = write(
            dir,
            "d1.iso",
            &plain(&disc_header(GameCube, "GALE01", 0, 0), 0x100),
        );
        let disc2 = write(
            dir,
            "d2.iso",
            &plain(&disc_header(GameCube, "GALE01", 1, 0), 0x100),
        );
        let rev1 = write(
            dir,
            "r1.gcm",
            &plain(&disc_header(GameCube, "GALE01", 0, 1), 0x100),
        );
        let rvz = write(
            dir,
            "d1.rvz",
            &wia(&disc_header(GameCube, "GALE01", 0, 0), true),
        );
        let tag = |path: &Path| state_content_tag(&GC, path);
        assert_eq!(tag(&disc1).as_deref(), Some("GALE01:0:0"));
        assert_eq!(tag(&disc2).as_deref(), Some("GALE01:1:0"));
        assert_eq!(tag(&rev1).as_deref(), Some("GALE01:0:1"));
        assert_eq!(tag(&rvz), tag(&disc1), "ISO and RVZ of one disc");
        let wii = write(
            dir,
            "w.iso",
            &plain(&disc_header(Wii, "RSPE01", 0, 0), 0x100),
        );
        assert_eq!(state_content_tag(&WII, &wii).as_deref(), Some("RSPE01:0:0"));
    }

    #[test]
    fn playlist_is_tagged_apart_from_its_first_disc() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        write(
            dir,
            "d1.iso",
            &plain(&disc_header(GameCube, "GALE01", 0, 0), 0x100),
        );
        write(
            dir,
            "d2.iso",
            &plain(&disc_header(GameCube, "GALE01", 1, 0), 0x100),
        );
        let m3u = write(dir, "game.m3u", b"d1.iso\nd2.iso\n");
        assert_eq!(state_content_tag(&GC, &m3u).as_deref(), Some("m3u:GALE01"));
    }

    #[test]
    fn executables_other_systems_and_unreadable_files_are_untagged() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let exe = write(dir, "a.dol", &dol(&[BLR], &[]));
        assert_eq!(state_content_tag(&GC, &exe), None);
        let sfc = write(dir, "a.sfc", &[0; 512]);
        assert_eq!(state_content_tag(&SNES, &sfc), None);
        let junk = write(dir, "junk.iso", &[0; 0x100]);
        assert_eq!(state_content_tag(&GC, &junk), None);
        assert_eq!(state_content_tag(&GC, &dir.join("gone.iso")), None);
    }
}
