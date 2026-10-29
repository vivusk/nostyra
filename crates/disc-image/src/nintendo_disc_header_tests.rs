//! GameCube / Wii disc header tests: every container fixture gives the right kind and
//! ID inside the read budget (counting `Read + Seek` wrapper); foreign `.iso` files
//! are not claimed; damaged containers say why.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::io::{Cursor, Read, Seek, SeekFrom};

use super::*;
use crate::test_images::{iso_sectors, to_iso, write, US_LICENCE};
use crate::test_nintendo_images::{ciso, disc_header, gcz, plain, wbfs, wia};
use crate::test_psp_images::psp_iso_sectors;

const GC: NintendoDiscKind = NintendoDiscKind::GameCube;
const WII: NintendoDiscKind = NintendoDiscKind::Wii;

/// Counts the bytes handed out and the seeks made by the wrapped reader.
struct CountingReader<R> {
    inner: R,
    bytes: u64,
    seeks: u32,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.bytes += n as u64;
        Ok(n)
    }
}

impl<R: Seek> Seek for CountingReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.seeks += 1;
        self.inner.seek(pos)
    }
}

/// Header of `image` read as extension `ext`, asserting the I/O budget.
fn header_of(image: &[u8], ext: &str) -> Result<Option<NintendoDiscHeader>, DiscError> {
    let mut reader = CountingReader {
        inner: Cursor::new(image),
        bytes: 0,
        seeks: 0,
    };
    let result = read_nintendo_disc_header_from(&mut reader, ext, image.len() as u64);
    assert!(
        reader.bytes <= MAX_HEADER_READ_BYTES,
        "{ext}: read {} bytes",
        reader.bytes
    );
    assert!(reader.seeks <= 2, "{ext}: {} seeks", reader.seeks);
    result
}

fn id_and_kind(image: &[u8], ext: &str) -> (String, NintendoDiscKind) {
    let header = header_of(image, ext).unwrap().expect(ext);
    (header.game_id_text().to_owned(), header.kind)
}

#[test]
fn plain_images_give_kind_id_disc_and_revision() {
    let gc = plain(&disc_header(GC, "GALE01", 1, 2), 0x1000);
    let header = header_of(&gc, "gcm").unwrap().unwrap();
    assert_eq!(header.game_id_text(), "GALE01");
    assert_eq!((header.disc, header.revision, header.kind), (1, 2, GC));
    let wii = plain(&disc_header(WII, "RJ2E01", 0, 0), 0x1000);
    assert_eq!(id_and_kind(&wii, "ISO"), ("RJ2E01".to_owned(), WII));
}

#[test]
fn foreign_and_short_files_are_not_claimed() {
    let ps2 = to_iso(&iso_sectors("PLAYSTATION", Some(US_LICENCE), &[]));
    assert!(
        header_of(&ps2, "iso").unwrap().is_none(),
        "PS2-like ISO9660"
    );
    let psp = to_iso(&psp_iso_sectors());
    assert!(header_of(&psp, "iso").unwrap().is_none(), "PSP UMD");
    let header = disc_header(GC, "GALE01", 0, 0);
    assert!(
        header_of(&header[..0x7F], "iso").unwrap().is_none(),
        "short file"
    );
    assert!(header_of(b"", "gcm").unwrap().is_none(), "empty file");
}

#[test]
fn damaged_game_id_is_a_format_error() {
    let mut header = disc_header(GC, "GALE01", 0, 0);
    header[3] = b'e';
    let err = header_of(&plain(&header, 0x200), "iso").unwrap_err();
    assert_eq!(err.to_string(), "damaged disc header");
}

#[test]
fn every_container_gives_the_header_within_budget() {
    let gc = disc_header(GC, "GALE01", 0, 0);
    let wii = disc_header(WII, "RJ2E01", 0, 1);
    let cases: [(Vec<u8>, &str, &str, NintendoDiscKind); 10] = [
        (ciso(&gc, true), "ciso", "GALE01", GC),
        (ciso(&wii, true), "CISO", "RJ2E01", WII),
        (wia(&gc, false), "wia", "GALE01", GC),
        (wia(&wii, true), "rvz", "RJ2E01", WII),
        (wbfs(&wii, 9), "wbfs", "RJ2E01", WII),
        (wbfs(&wii, 12), "wbfs", "RJ2E01", WII),
        (gcz(&gc, 1, false), "gcz", "GALE01", GC),
        (gcz(&gc, 3, false), "gcz", "GALE01", GC),
        (gcz(&wii, 3, true), "gcz", "RJ2E01", WII),
        (plain(&wii, 0x10000), "gcm", "RJ2E01", WII),
    ];
    for (image, ext, id, kind) in cases {
        assert_eq!(id_and_kind(&image, ext), (id.to_owned(), kind), "{ext}");
    }
}

#[test]
fn the_extension_picks_the_container() {
    let gc = disc_header(GC, "GALE01", 0, 0);
    // A CISO read as plain (PSP `.cso` dispatch) or a plain disc read as CISO: none.
    assert!(header_of(&ciso(&gc, true), "iso").unwrap().is_none());
    assert!(header_of(&plain(&gc, 0x9000), "ciso").unwrap().is_none());
    assert!(header_of(&wia(&gc, false), "gcz").unwrap().is_none());
    // WBFS holds Wii discs only.
    assert!(header_of(&wbfs(&gc, 9), "wbfs").unwrap().is_none());
    // CISO whose block 0 is unused stores no header.
    assert!(header_of(&ciso(&gc, false), "ciso").unwrap().is_none());
}

#[test]
fn damaged_containers_say_why() {
    let wii = disc_header(WII, "RJ2E01", 0, 0);
    let text = |image: &[u8], ext: &str| header_of(image, ext).unwrap_err().to_string();
    let ciso_image = ciso(&wii, true);
    assert_eq!(
        text(&ciso_image[..0x8040], "ciso"),
        "CISO image is truncated"
    );
    let mut bad_shift = wbfs(&wii, 9);
    bad_shift[8] = 31;
    assert_eq!(text(&bad_shift, "wbfs"), "damaged WBFS header");
    let mut empty = wbfs(&wii, 9);
    empty[12] = 0;
    assert_eq!(text(&empty, "wbfs"), "WBFS image holds no disc");
    let image = gcz(&wii, 1, false);
    let mut huge = image.clone();
    huge[8..16].copy_from_slice(&(1u64 << 32).to_le_bytes()); // compressed size
    assert_eq!(text(&huge, "gcz"), "GCZ block is too large");
    let mut garbage = image.clone();
    let data_at = 32 + 12;
    garbage[data_at..data_at + 2].copy_from_slice(b"\xFF\xFF"); // zlib header
    assert_eq!(text(&garbage, "gcz"), "GCZ block does not decompress");
    assert_eq!(
        text(&image[..image.len() - 1], "gcz"),
        "GCZ image is truncated"
    );
    let two_blocks = gcz(&wii, 2, false);
    assert_eq!(text(&two_blocks[..44], "gcz"), "GCZ image is truncated");
}

#[test]
fn path_reader_ignores_other_extensions() {
    let dir = tempfile::tempdir().unwrap();
    let gc = plain(&disc_header(GC, "GALE01", 0, 0), 0x200);
    let iso = write(dir.path(), "game.iso", &gc);
    let header = read_nintendo_disc_header(&iso).unwrap().unwrap();
    assert_eq!(header.game_id_text(), "GALE01");
    let bin = write(dir.path(), "game.bin", &gc);
    assert!(read_nintendo_disc_header(&bin).unwrap().is_none());
}
