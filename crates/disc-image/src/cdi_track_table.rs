//! The session/track table of a DiscJuggler image, byte for byte as Flycast's
//! `cdipsr` reads it (`CDI_read_track`, `CDI_skip_next_session`). Track data follows
//! the table order from byte 0 of the image.

use crate::cdi_image::{CdiImage, CdiTrack};

const TRACK_MARK: [u8; 10] = [0, 0, 1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];

/// Little-endian cursor over the table; every read is bounds-checked.
struct Cursor<'a>(&'a [u8]);

impl Cursor<'_> {
    fn skip(&mut self, n: usize) -> Option<()> {
        self.0 = self.0.get(n..)?;
        Some(())
    }
    fn bytes(&mut self, n: usize) -> Option<&[u8]> {
        let (head, tail) = (self.0.get(..n)?, self.0.get(n..)?);
        self.0 = tail;
        Some(head)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.bytes(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }
}

pub(crate) fn parse_table(table: &[u8], v3: bool) -> Option<CdiImage> {
    let mut c = Cursor(table);
    let sessions = c.u16()?;
    let mut image = CdiImage::default();
    let mut position = 0u64;
    for session in 0..sessions {
        let tracks = c.u16()?;
        if tracks > 99 {
            return None;
        }
        for _ in 0..tracks {
            let (track, total) = parse_track(&mut c, v3, session, position)?;
            position += u64::from(total) * u64::from(track.sector_size);
            image.tracks.push(track);
        }
        c.skip(12 + usize::from(v3))?; // session trailer
    }
    Some(image)
}

/// One track descriptor; returns the track and its stored length (pregap + data).
fn parse_track(c: &mut Cursor, v3: bool, session: u16, position: u64) -> Option<(CdiTrack, u32)> {
    if c.u32()? != 0 {
        c.skip(8)?; // extra data (DiscJuggler 3.00.780+)
    }
    if c.bytes(10)? != TRACK_MARK || c.bytes(10)? != TRACK_MARK {
        return None;
    }
    c.skip(4)?;
    let name_len = c.u8()?;
    c.skip(usize::from(name_len) + 11 + 4 + 4)?;
    if c.u32()? == 0x8000_0000 {
        c.skip(8)?; // DiscJuggler 4
    }
    c.skip(2)?;
    let (pregap, length) = (c.u32()?, c.u32()?);
    c.skip(6)?;
    let mode = c.u32()?;
    c.skip(12)?;
    let (start_lba, total) = (c.u32()?, c.u32()?);
    c.skip(16)?;
    let sector_size = match c.u32()? {
        0 => 2048,
        1 => 2336,
        2 => 2352,
        4 => 2448,
        _ => return None,
    };
    c.skip(29)?;
    if v3 {
        c.skip(5)?;
        if c.u32()? == 0xFFFF_FFFF {
            c.skip(78)?; // extra data (DiscJuggler 3.00.780+)
        }
    }
    let valid = mode <= 2 && total >= pregap.checked_add(length)?;
    valid.then(|| {
        let track = CdiTrack {
            session,
            mode,
            sector_size,
            lba: start_lba + pregap,
            length,
            data_start: position + u64::from(pregap) * u64::from(sector_size),
        };
        (track, total)
    })
}
