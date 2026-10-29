//! Track lists, selectors and absolute-LBA reads per container (synthetic images).

use super::*;
use crate::test_cdi_image::{cdi_image, CDI_V35};
use crate::test_gd_images::{gd_chd, write_gdi, write_redump_gd_cue};
use crate::test_images::{iso_sectors, to_chd, to_iso, to_raw, write};
use crate::test_psp_containers::{cso, dvd_chd};

const KATANA: &[u8] = b"SEGA SEGAKATANA ";

fn summary(tracks: &[TrackInfo]) -> Vec<(u32, u16, bool, u32)> {
    tracks
        .iter()
        .map(|t| (t.number, t.session, t.data, t.start_lba))
        .collect()
}

/// Opens `selector` and returns (first sector, first 16 bytes there).
fn head(path: &Path, selector: TrackSelector) -> (u32, Vec<u8>) {
    let mut track = DiscTrack::open(path, selector).unwrap();
    let first = track.first_track_sector();
    let mut buf = [0u8; 16];
    assert_eq!(track.read_user_data_abs(first, &mut buf), 16);
    (first, buf.to_vec())
}

/// Standard identifier of the ISO9660 primary volume descriptor (LBA 16).
fn pvd_id(path: &Path, selector: TrackSelector) -> [u8; 5] {
    let mut track = DiscTrack::open(path, selector).unwrap();
    let mut buf = [0u8; 6];
    assert_eq!(track.read_user_data_abs(16, &mut buf), 6);
    [buf[1], buf[2], buf[3], buf[4], buf[5]]
}

#[test]
fn redump_gd_cue_puts_the_high_density_area_at_45000() {
    let dir = tempfile::tempdir().unwrap();
    let cue = write_redump_gd_cue(dir.path(), "Game", "T-0001");
    let tracks = track_list(&cue).unwrap();
    assert_eq!(
        summary(&tracks),
        [(1, 0, true, 0), (2, 0, false, 4), (3, 0, true, 45_000)]
    );
    assert_eq!(tracks[2].sector_count, 9);
    assert_eq!(
        head(&cue, TrackSelector::Number(3)),
        (45_000, KATANA.to_vec())
    );
    assert_eq!(head(&cue, TrackSelector::Largest).0, 45_000);
    assert_eq!(head(&cue, TrackSelector::Last).0, 45_000);
    assert_eq!(head(&cue, TrackSelector::FirstData).0, 0);
    assert!(matches!(
        DiscTrack::open(&cue, TrackSelector::Number(2)),
        Err(DiscError::NoDataTrack)
    ));
}

#[test]
fn gdi_reads_by_absolute_lba_and_spans_sectors() {
    let dir = tempfile::tempdir().unwrap();
    let gdi = write_gdi(dir.path(), "Game", "T-0002", true);
    let tracks = track_list(&gdi).unwrap();
    assert_eq!(
        summary(&tracks),
        [(1, 0, true, 0), (2, 0, false, 154), (3, 1, true, 45_000)]
    );
    let mut track = DiscTrack::open(&gdi, TrackSelector::FirstOfSecondSession).unwrap();
    assert_eq!(track.info().number, 3);
    let mut buf = vec![0u8; 2048 * 2 + 10];
    assert_eq!(track.read_user_data_abs(45_000, &mut buf), buf.len());
    assert_eq!(&buf[..16], KATANA);
    assert_eq!(
        track.read_user_data_abs(44_999, &mut buf),
        0,
        "before the track"
    );
    let end = 45_000 + track.info().sector_count as u32;
    assert_eq!(
        track.read_user_data_abs(end - 1, &mut buf),
        2048,
        "short at the end"
    );
}

#[test]
fn gd_chd_track_3_starts_at_45000() {
    let dir = tempfile::tempdir().unwrap();
    let chd = write(dir.path(), "game.chd", &gd_chd("T-0003"));
    let tracks = track_list(&chd).unwrap();
    assert_eq!(
        summary(&tracks),
        [(1, 0, true, 0), (2, 0, false, 5), (3, 1, true, 45_000)]
    );
    assert_eq!(
        head(&chd, TrackSelector::Number(3)),
        (45_000, KATANA.to_vec())
    );
}

#[test]
fn cd_chd_and_dvd_chd_are_single_data_tracks_from_0() {
    let dir = tempfile::tempdir().unwrap();
    let sectors = iso_sectors("PLAYSTATION", None, &[]);
    let cd = write(
        dir.path(),
        "ps1.chd",
        &to_chd(&to_raw(&sectors, 2), "MODE2_RAW"),
    );
    let dvd = write(dir.path(), "psp.chd", &dvd_chd(&to_iso(&sectors), true));
    for path in [&cd, &dvd] {
        let tracks = track_list(path).unwrap();
        assert_eq!(summary(&tracks), [(1, 0, true, 0)], "{}", path.display());
        assert_eq!(tracks[0].sector_count, sectors.len() as u64);
        assert_eq!(pvd_id(path, TrackSelector::FirstData), *b"CD001");
    }
}

#[test]
fn cdi_second_session_track_carries_its_table_lba() {
    let dir = tempfile::tempdir().unwrap();
    let cdi = write(dir.path(), "game.cdi", &cdi_image(CDI_V35, "T-0004"));
    let tracks = track_list(&cdi).unwrap();
    assert_eq!(summary(&tracks), [(1, 0, false, 0), (2, 1, true, 11_702)]);
    assert_eq!(
        head(&cdi, TrackSelector::FirstOfSecondSession),
        (11_702, KATANA.to_vec())
    );
    assert!(DiscTrack::open(&cdi, TrackSelector::Number(3)).is_err());
}

#[test]
fn iso_cso_and_raw_bin_are_track_1_at_0() {
    let dir = tempfile::tempdir().unwrap();
    let sectors = iso_sectors("PSP GAME", None, &[]);
    let iso = write(dir.path(), "game.iso", &to_iso(&sectors));
    let cso = write(dir.path(), "game.cso", &cso(&to_iso(&sectors), 2048, 0));
    let bin = write(dir.path(), "game.bin", &to_raw(&sectors, 1));
    for path in [&iso, &cso, &bin] {
        let tracks = track_list(path).unwrap();
        assert_eq!(summary(&tracks), [(1, 0, true, 0)], "{}", path.display());
        assert_eq!(tracks[0].sector_count, sectors.len() as u64);
        assert_eq!(pvd_id(path, TrackSelector::Largest), *b"CD001");
        assert!(DiscTrack::open(path, TrackSelector::Number(2)).is_err());
    }
}

#[test]
fn largest_keeps_the_first_of_equal_tracks_and_skips_audio() {
    let track = |number, data, sector_count| TrackInfo {
        number,
        session: 0,
        data,
        start_lba: number * 100,
        sector_count,
    };
    let tracks = [
        track(1, true, 5),
        track(2, false, 50),
        track(3, true, 9),
        track(4, true, 9),
    ];
    assert_eq!(
        TrackSelector::Largest.pick(&tracks).map(|t| t.number),
        Some(3)
    );
    assert_eq!(TrackSelector::Last.pick(&tracks).map(|t| t.number), Some(4));
    assert_eq!(TrackSelector::FirstOfSecondSession.pick(&tracks), None);
}

#[test]
fn playlists_and_eboots_are_not_disc_images() {
    for name in ["game.m3u", "EBOOT.PBP"] {
        assert!(matches!(
            track_list(Path::new(name)),
            Err(DiscError::Format(_))
        ));
    }
}
