//! LBA math past 4 GiB through the real readers over [`VirtualDisc`] (no multi-GiB
//! file): `FileTrackReader::open_auto` and `DiscTrack::open` return the marker sector
//! at LBA 2 097 157 for a plain `.iso` and a raw `.bin`; the DVD CHD half is the
//! pure `chd_dvd_reader::hunk_of` (a real > 4 GiB CHD would need ~1 M hunk-map
//! entries). `with_source` is thread-local: each test installs its own source.

use std::rc::Rc;

use content_file_source::with_source;

use super::{VirtualDisc, MARKER, MARKER_LBA, SECTORS};
use crate::chd_dvd_reader::hunk_of;
use crate::disc_sector_reader::{DiscReader, FileTrackReader};
use crate::{DiscError, DiscTrack, TrackSelector};

/// The marker byte offset lies past 4 GiB in both layouts.
#[test]
fn marker_offset_needs_more_than_32_bits() {
    for disc in [VirtualDisc::iso(), VirtualDisc::raw_bin()] {
        let at = disc.marker_offset();
        assert!(at > u64::from(u32::MAX), "{at}");
        assert!(disc.len_bytes() > at + 2048);
    }
}

#[test]
fn file_track_reader_reads_the_marker_sector_of_both_layouts() {
    for disc in [VirtualDisc::iso(), VirtualDisc::raw_bin()] {
        let path = disc.path().to_path_buf();
        let (sectors, data, below) = with_source(Rc::new(disc), || {
            let mut reader = FileTrackReader::open_auto(&path).unwrap();
            let data = reader.read_user_data(MARKER_LBA).unwrap();
            let below = reader.read_user_data(MARKER_LBA - 1).unwrap();
            (reader.sector_count(), data, below)
        });
        assert_eq!(sectors, SECTORS, "{}", path.display());
        assert_eq!(data[..16], MARKER, "{}", path.display());
        assert!(data[16..].iter().all(|&b| b == 0));
        assert!(below.iter().all(|&b| b == 0), "neighbour sector is zero");
    }
}

#[test]
fn disc_track_reads_the_marker_by_absolute_lba_and_spans_into_the_next_sector() {
    for disc in [VirtualDisc::iso(), VirtualDisc::raw_bin()] {
        let path = disc.path().to_path_buf();
        let (info, buf, past_end) = with_source(Rc::new(disc), || {
            let mut track = DiscTrack::open(&path, TrackSelector::FirstData).unwrap();
            let mut buf = vec![0xAAu8; 2048 + 16];
            let n = track.read_user_data_abs(MARKER_LBA, &mut buf);
            assert_eq!(n, buf.len());
            let mut tail = [0u8; 8];
            let past_end = track.read_user_data_abs(SECTORS as u32, &mut tail);
            (track.info().clone(), buf, past_end)
        });
        assert_eq!((info.start_lba, info.sector_count), (0, SECTORS));
        assert_eq!(buf[..16], MARKER, "{}", path.display());
        assert!(buf[16..].iter().all(|&b| b == 0), "rest + next sector zero");
        assert_eq!(past_end, 0, "nothing past the image");
    }
}

#[test]
fn hunk_of_uses_64_bit_byte_offsets() {
    let byte = u64::from(MARKER_LBA) * 2048;
    for hunk_bytes in [4096u64, 19_584] {
        let want = ((byte / hunk_bytes) as u32, (byte % hunk_bytes) as usize);
        assert_eq!(
            hunk_of(MARKER_LBA, hunk_bytes).unwrap(),
            want,
            "{hunk_bytes}"
        );
    }
    assert_eq!(hunk_of(0, 4096).unwrap(), (0, 0));
    assert_eq!(hunk_of(3, 4096).unwrap(), (1, 2048));
    assert!(matches!(
        hunk_of(u32::MAX, 1),
        Err(DiscError::SectorOutOfRange(u32::MAX))
    ));
}
