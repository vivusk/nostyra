//! rc_hash cdreader over `disc_image::DiscTrack`: tracks of cue/bin, GDI, CHD (CD /
//! GD-ROM / DVD), CSO and CDI images by number or rcheevos special pick, read at
//! absolute LBAs (GD-ROM high-density area at 45000). rcheevos' default reader knows
//! only cue / gdi / bare bin and misses Redump's GD 45000 offset (M15 D9).
//! All five callbacks are set: `rc_hash_merge_callbacks` (rc_client path) copies the
//! cdreader only when the legacy `open_track` is non-NULL.

use std::ffi::{c_char, c_void};

use disc_image::{DiscTrack, TrackSelector};

use super::hash_ffi::{RcHashCdreader, RcHashIterator};
use super::hash_file_reader::path_of;

pub fn cd_reader() -> RcHashCdreader {
    RcHashCdreader {
        open_track: Some(cd_open_track),
        read_sector: Some(cd_read_sector),
        close_track: Some(cd_close_track),
        first_track_sector: Some(cd_first_track_sector),
        open_track_iterator: Some(cd_open_track_iterator),
    }
}

/// rcheevos track argument: -1 first data, -2 last, -3 (or legacy 0) largest, -4
/// first of the second session, else a track number.
fn rc_track_selector(track: u32) -> TrackSelector {
    match track {
        u32::MAX => TrackSelector::FirstData,
        0xFFFF_FFFE => TrackSelector::Last,
        0 | 0xFFFF_FFFD => TrackSelector::Largest,
        0xFFFF_FFFC => TrackSelector::FirstOfSecondSession,
        n => TrackSelector::Number(n),
    }
}

unsafe extern "C" fn cd_open_track(path: *const c_char, track: u32) -> *mut c_void {
    // SAFETY: rcheevos passes a NUL-terminated UTF-8 path valid for the call.
    let Some(path) = (unsafe { path_of(path) }) else {
        return std::ptr::null_mut();
    };
    let selector = rc_track_selector(track);
    match DiscTrack::open(path, selector) {
        Ok(opened) => Box::into_raw(Box::new(opened)).cast(),
        Err(e) => {
            // rcheevos probes tracks that may not exist (Dreamcast: track 3 first).
            tracing::debug!(target: "rcheevos", path = %path.display(), ?selector, "cd track not opened: {e}");
            std::ptr::null_mut()
        }
    }
}

unsafe extern "C" fn cd_open_track_iterator(
    path: *const c_char,
    track: u32,
    _iterator: *const RcHashIterator,
) -> *mut c_void {
    // SAFETY: same contract as `cd_open_track`.
    unsafe { cd_open_track(path, track) }
}

/// # Safety
/// `handle` is NULL or came from `cd_open_track` and is not closed yet.
unsafe fn track<'a>(handle: *mut c_void) -> Option<&'a mut DiscTrack> {
    // SAFETY: caller contract.
    unsafe { handle.cast::<DiscTrack>().as_mut() }
}

unsafe extern "C" fn cd_read_sector(
    handle: *mut c_void,
    sector: u32,
    buffer: *mut c_void,
    requested: usize,
) -> usize {
    if buffer.is_null() || requested == 0 {
        return 0;
    }
    // SAFETY: live handle from `cd_open_track` (or NULL).
    let Some(track) = (unsafe { track(handle) }) else {
        return 0;
    };
    // SAFETY: rcheevos hands a writable buffer of `requested` bytes.
    let out = unsafe { std::slice::from_raw_parts_mut(buffer.cast::<u8>(), requested) };
    track.read_user_data_abs(sector, out)
}

unsafe extern "C" fn cd_first_track_sector(handle: *mut c_void) -> u32 {
    // SAFETY: live handle from `cd_open_track` (or NULL).
    unsafe { track(handle) }.map_or(0, |t| t.first_track_sector())
}

unsafe extern "C" fn cd_close_track(handle: *mut c_void) {
    if !handle.is_null() {
        // SAFETY: handle from `cd_open_track`, closed once.
        drop(unsafe { Box::from_raw(handle.cast::<DiscTrack>()) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rcheevos_special_tracks_map_to_selectors() {
        assert_eq!(rc_track_selector(u32::MAX), TrackSelector::FirstData);
        assert_eq!(rc_track_selector(u32::MAX - 1), TrackSelector::Last);
        assert_eq!(rc_track_selector(u32::MAX - 2), TrackSelector::Largest);
        assert_eq!(rc_track_selector(0), TrackSelector::Largest);
        assert_eq!(
            rc_track_selector(u32::MAX - 3),
            TrackSelector::FirstOfSecondSession
        );
        assert_eq!(rc_track_selector(3), TrackSelector::Number(3));
    }
}
