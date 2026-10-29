"""GD-ROM writers for the Nostyra Dreamcast test discs: IP.BIN header, a tiny
ISO9660 volume holding 1ST_READ.BIN, the TOSEC-style GDI and the Redump-style cue.

Part of build-nostyra-test-dreamcast.py (self-written, GPLv3, Python 3 std-only).
"""
import struct

SECTOR = 2048
HD_LBA = 45000  # GD-ROM high-density area: track 3, IP.BIN in its first 16 sectors


def both16(v):
    return struct.pack("<H", v) + struct.pack(">H", v)


def both32(v):
    return struct.pack("<I", v) + struct.pack(">I", v)


def pad(text, size):
    return text.encode("ascii").ljust(size, b" ")


def ip_bin(product, title):
    """IP.BIN header (first 256 bytes of the high-density area); no bootstrap code:
    reios loads 1ST_READ.BIN itself."""
    meta = pad("SEGA SEGAKATANA", 16) + pad("SEGA ENTERPRISES", 16)
    meta += pad("0000 GD-ROM1/1", 16) + pad("JUE", 8) + pad("E000F10", 8)
    meta += pad(product, 10) + pad("V1.000", 6) + pad("20261026", 16)
    meta += pad("1ST_READ.BIN", 16) + pad("NOSTYRA", 16) + pad(title, 128)
    assert len(meta) == 256
    return meta.ljust(16 * SECTOR, b"\0")


def dir_record(name, lba, size, is_dir):
    rec = struct.pack("<BB", 0, 0) + both32(lba) + both32(size)
    rec += bytes([126, 10, 26, 0, 0, 0, 0]) + bytes([2 if is_dir else 0, 0, 0]) + both16(1)
    rec += bytes([len(name)]) + name
    rec += b"\0" * (len(rec) % 2)
    return bytes([len(rec)]) + rec[1:]


def iso_area(boot):
    """Sectors 16.. of track 3: PVD, terminator, root directory, 1ST_READ.BIN.
    ISO9660 extents on a GD-ROM are absolute disc LBAs."""
    root_lba, file_lba = HD_LBA + 18, HD_LBA + 19
    file_sectors = (len(boot) + SECTOR - 1) // SECTOR
    total = 19 + file_sectors
    pvd = bytes([1]) + b"CD001" + bytes([1, 0]) + pad("SEGA SEGAKATANA", 32) + pad("NOSTYRA", 32)
    pvd += bytes(8) + both32(HD_LBA + total) + bytes(32) + both16(1) + both16(1) + both16(SECTOR)
    pvd += both32(0) + bytes(16)
    pvd += dir_record(b"\0", root_lba, SECTOR, True)
    pvd = pvd.ljust(SECTOR, b"\0")
    term = (bytes([255]) + b"CD001" + bytes([1])).ljust(SECTOR, b"\0")
    root = dir_record(b"\0", root_lba, SECTOR, True) + dir_record(b"\1", root_lba, SECTOR, True)
    root += dir_record(b"1ST_READ.BIN;1", file_lba, len(boot), False)
    root = root.ljust(SECTOR, b"\0")
    return pvd + term + root + boot.ljust(file_sectors * SECTOR, b"\0")


def gdi_disc(out_dir, stem, product, title, boot):
    """Three-track GDI (TOSEC layout, quoted names): data track 1, audio track 2, data
    track 3 at LBA 45000 holding IP.BIN + an ISO9660 volume with 1ST_READ.BIN.
    Returns the track names."""
    tracks = [
        (f"{stem} (Track 1).bin", 0, 4, SECTOR, bytes(2 * SECTOR)),
        (f"{stem} (Track 2).raw", 152, 0, 2352, bytes(4 * 2352)),
        (f"{stem} (Track 3).bin", HD_LBA, 4, SECTOR, ip_bin(product, title) + iso_area(boot)),
    ]
    lines = [f'{n + 1} {lba} {ctrl} {size} "{name}" 0' for n, (name, lba, ctrl, size, _) in enumerate(tracks)]
    for name, _, _, _, data in tracks:
        (out_dir / name).write_bytes(data)
    (out_dir / f"{stem}.gdi").write_text("3\n" + "\n".join(lines) + "\n", newline="\n")
    return [name for name, *_ in tracks]


def redump_cue(names):
    """The GDI's tracks as a Redump-style sheet: single-density then high-density area."""
    modes = ["MODE1/2048", "AUDIO", "MODE1/2048"]
    files = [f'FILE "{name}" BINARY\n  TRACK {n + 1:02d} {modes[n]}\n    INDEX 01 00:00:00\n' for n, name in enumerate(names)]
    return "REM SINGLE-DENSITY AREA\n" + files[0] + files[1] + "REM HIGH-DENSITY AREA\n" + files[2]
