//! The rcheevos cdreader past 4 GiB (PlayStation 2 DVD-9 discs): `cd_open_track` /
//! `cd_read_sector` over `disc_image`'s `VirtualDisc` (a zero-filled > 4 GiB image
//! served from memory under `content_file_source::with_source`, nothing on disk)
//! return the marker sector at LBA 2 097 157 for a plain `.iso` and a raw `.bin` —
//! no `u32` wrap anywhere between the C shim and the file offset.

use std::ffi::CString;
use std::rc::Rc;

use content_file_source::with_source;
use disc_image::virtual_disc_source::{VirtualDisc, MARKER, MARKER_LBA, SECTORS};

use super::{cd_close_track, cd_first_track_sector, cd_open_track, cd_read_sector};

fn marker_sector_through_the_ffi(disc: VirtualDisc) -> (u32, Vec<u8>, usize) {
    let path = CString::new(disc.path().to_str().unwrap()).unwrap();
    with_source(Rc::new(disc), || {
        // SAFETY: a NUL-terminated path; the handle is used and closed once here.
        unsafe {
            let handle = cd_open_track(path.as_ptr(), u32::MAX);
            assert!(!handle.is_null(), "virtual disc opened as track");
            let first = cd_first_track_sector(handle);
            let mut buf = vec![0xAAu8; 2048];
            let n = cd_read_sector(handle, MARKER_LBA, buf.as_mut_ptr().cast(), buf.len());
            let mut tail = [0u8; 16];
            let past = cd_read_sector(handle, SECTORS as u32, tail.as_mut_ptr().cast(), 16);
            cd_close_track(handle);
            assert_eq!(n, 2048);
            (first, buf, past)
        }
    })
}

#[test]
fn iso_and_raw_bin_marker_sectors_read_past_4_gib() {
    for disc in [VirtualDisc::iso(), VirtualDisc::raw_bin()] {
        let label = disc.path().display().to_string();
        let (first, data, past_end) = marker_sector_through_the_ffi(disc);
        assert_eq!(first, 0, "{label}");
        assert_eq!(data[..16], MARKER, "{label}");
        assert!(data[16..].iter().all(|&b| b == 0), "{label}");
        assert_eq!(past_end, 0, "{label}: nothing past the image");
    }
}
