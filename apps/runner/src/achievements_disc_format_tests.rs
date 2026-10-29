//! Disc formats reach the same RetroAchievements game (M15 phase 07): every launch form
//! of the PS1 / Dreamcast / PSP test discs (fixtures, plus CHD / CDI containers built in
//! a temp dir with `disc-image`'s `test-images`) starts a session whose hash the fake
//! server knows, so the canned set loads; another disc's hash stays unknown. No core:
//! a zeroed SYSTEM_RAM buffer stands in for memory (the set loads in the first frame).
//! The Dreamcast GD-ROM CHD is covered by the hash parity test of the achievements
//! crate (it needs that crate's CHD v5 track builder).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::cell::Cell;
use std::path::Path;

use disc_image::test_cdi_image::{cdi_image_with, CDI_V35};
use disc_image::test_images::{to_chd, write};
use disc_image::test_psp_containers::dvd_chd;
use ipc_protocol::RunnerEvent;

use crate::achievements_fake_set_session::{start_fake_set_session, CoreFacts, RaCase};
use crate::runner_real_core_suite::repo;

/// Starts `system`'s fake-set session for `image`, runs one frame with `ram_len` bytes
/// of system RAM and returns the game id the set loaded for (None: unknown game).
fn loaded_game(system: &str, image: &Path, ram_len: usize) -> Option<u32> {
    let case = RaCase::load(system);
    let data = tempfile::tempdir().unwrap();
    let facts = CoreFacts {
        library_name: "test core",
        version: "1.0",
        options: &[],
    };
    let mut started = start_fake_set_session(&case, image, data.path(), false, &facts);
    let ram: Vec<Cell<u8>> = (0..ram_len).map(|_| Cell::new(0)).collect();
    let lookup = |id: u32| (id == 2).then_some((ram.as_ptr() as *mut u8, ram.len()));
    let mut session = started.session.take().unwrap();
    session.on_real_frame(None, &lookup);
    session.shutdown();
    let events = started.events.borrow();
    events.iter().find_map(|event| match event {
        RunnerEvent::AchievementsLoaded { ra_game_id, .. } => Some(*ra_game_id),
        _ => None,
    })
}

fn roms(name: &str) -> std::path::PathBuf {
    repo(&format!("test-assets/roms/{name}"))
}

fn assert_identified(system: &str, image: &Path, ram_len: usize) {
    let game = RaCase::load(system).game_id;
    let loaded = loaded_game(system, image, ram_len);
    assert_eq!(loaded, Some(game), "{system}: {}", image.display());
}

#[test]
fn ps1_cue_bin_m3u_and_chd_are_one_game() {
    const RAM: usize = 0x20_0000;
    for name in [
        "nostyra-test-psx.cue",
        "nostyra-test-psx.bin",
        "nostyra-test-psx.m3u",
    ] {
        assert_identified("psx", &roms(name), RAM);
    }
    let dir = tempfile::tempdir().unwrap();
    let bin = std::fs::read(roms("nostyra-test-psx.bin")).unwrap();
    let chd = write(dir.path(), "psx.chd", &to_chd(&bin, "MODE2_RAW"));
    assert_identified("psx", &chd, RAM);
    let disc2 = roms("nostyra-test-psx-disc2.cue");
    assert_eq!(loaded_game("psx", &disc2, RAM), None, "other hash: unknown");
}

#[test]
fn dreamcast_gdi_cue_m3u_and_cdi_are_one_game() {
    const RAM: usize = 0x100_0000;
    for name in [
        "nostyra-test-dc.gdi",
        "nostyra-test-dc.cue",
        "nostyra-test-dc.m3u",
    ] {
        assert_identified("dreamcast", &roms(name), RAM);
    }
    let track = std::fs::read(roms("nostyra-test-dc (Track 3).bin")).unwrap();
    let sectors: Vec<Vec<u8>> = track.chunks(2048).map(<[u8]>::to_vec).collect();
    let dir = tempfile::tempdir().unwrap();
    // Data track at LBA 45000 so the fixture's absolute ISO9660 LBAs line up.
    let cdi = write(
        dir.path(),
        "dc.cdi",
        &cdi_image_with(CDI_V35, &sectors, 45_000 - 2),
    );
    assert_identified("dreamcast", &cdi, RAM);
}

#[test]
fn psp_iso_cso_and_chd_are_one_game() {
    const RAM: usize = 0x200_0000;
    for name in ["nostyra-test-psp.iso", "nostyra-test-psp.cso"] {
        assert_identified("psp", &roms(name), RAM);
    }
    let dir = tempfile::tempdir().unwrap();
    let iso = std::fs::read(roms("nostyra-test-psp.iso")).unwrap();
    let chd = write(dir.path(), "psp.chd", &dvd_chd(&iso, true));
    assert_identified("psp", &chd, RAM);
}
