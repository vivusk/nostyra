"""Container writers for the Nostyra PSP test content: PARAM.SFO, a UMD-style ISO9660
image (PVD "PSP GAME", UMD_DATA.BIN, PSP_GAME/PARAM.SFO, PSP_GAME/SYSDIR/EBOOT.BIN),
CSO v1 (raw deflate frames) and a homebrew EBOOT.PBP.

Part of build-nostyra-test-psp.py (self-written, GPLv3, Python 3 std-only).
"""
import struct
import zlib

SECTOR = 2048


def param_sfo(entries):
    """PSF v1.1: `entries` = {key: str | int}; strings utf-8 NUL-terminated (fmt 0x0204),
    ints u32 (fmt 0x0404). Keys sorted as the PSP does."""
    keys, data, index = bytearray(), bytearray(), bytearray()
    for key in sorted(entries):
        value = entries[key]
        if isinstance(value, int):
            fmt, raw, max_len = 0x0404, struct.pack("<I", value), 4
        else:
            fmt, raw = 0x0204, value.encode("utf-8") + b"\0"
            max_len = (len(raw) + 3) // 4 * 4
        index += struct.pack("<HHIII", len(keys), fmt, len(raw), max_len, len(data))
        keys += key.encode("ascii") + b"\0"
        data += raw.ljust(max_len, b"\0")
    keys += b"\0" * (-len(keys) % 4)
    key_start = 20 + len(index)
    head = b"\0PSF" + struct.pack("<IIII", 0x0101, key_start, key_start + len(keys), len(entries))
    return head + bytes(index) + bytes(keys) + bytes(data)


def both(fmt, value):
    """ISO9660 both-endian field: little-endian then big-endian."""
    return struct.pack("<" + fmt, value) + struct.pack(">" + fmt, value)


def dir_record(lba, size, name, is_dir=False):
    rec = bytearray(33 + len(name) + (len(name) + 1) % 2)
    rec[0] = len(rec)
    rec[2:18] = both("I", lba) + both("I", size)
    rec[18:25] = bytes([126, 10, 27, 0, 0, 0, 0])  # 2026-10-27
    rec[25] = 2 if is_dir else 0
    rec[28:32] = both("H", 1)  # volume sequence number
    rec[32] = len(name)
    rec[33:33 + len(name)] = name
    return bytes(rec)


def iso_image(tree, system_id="PSP GAME", volume_id="NOSTYRA_TEST_PSP"):
    """ISO9660 image of `tree` ({name: bytes | dict}); one sector per directory,
    files sector-aligned after the directories. No path table (PPSSPP walks records)."""
    dirs, files = [], []  # dirs: (node, lba, parent_index); files: (data, lba)

    def collect(node, parent):
        dirs.append([node, 0, parent])
        me = len(dirs) - 1
        for name in sorted(node):
            if isinstance(node[name], dict):
                collect(node[name], me)
        return me

    collect(tree, 0)
    lba = 18
    for d in dirs:
        d[1], lba = lba, lba + 1
    file_lba = {}
    for d in dirs:
        for name in sorted(d[0]):
            data = d[0][name]
            if not isinstance(data, dict):
                file_lba[id(data)] = lba
                files.append((data, lba))
                lba += max(1, (len(data) + SECTOR - 1) // SECTOR)
    sectors = bytearray(lba * SECTOR)
    for index, (node, at, parent) in enumerate(dirs):
        rec = dir_record(at, SECTOR, b"\0", True) + dir_record(dirs[parent][1], SECTOR, b"\1", True)
        for name in sorted(node):
            child = node[name]
            if isinstance(child, dict):
                child_lba = next(d[1] for d in dirs if d[0] is child)
                rec += dir_record(child_lba, SECTOR, name.encode(), True)
            else:
                rec += dir_record(file_lba[id(child)], len(child), f"{name};1".encode())
        assert len(rec) <= SECTOR
        sectors[at * SECTOR:at * SECTOR + len(rec)] = rec
    for data, at in files:
        sectors[at * SECTOR:at * SECTOR + len(data)] = data
    pvd = bytearray(SECTOR)
    pvd[0:8] = b"\x01CD001\x01\x00"
    pvd[8:40] = system_id.encode().ljust(32)
    pvd[40:72] = volume_id.encode().ljust(32)
    pvd[80:88] = both("I", lba)  # volume space size
    pvd[120:132] = both("H", 1) + both("H", 1) + both("H", SECTOR)  # set size, seq, block
    pvd[156:190] = dir_record(18, SECTOR, b"\0", True)
    pvd[881] = 1  # file structure version
    sectors[16 * SECTOR:17 * SECTOR] = pvd
    sectors[17 * SECTOR:17 * SECTOR + 7] = b"\xffCD001\x01"
    return bytes(sectors)


def cso_image(iso, block=SECTOR):
    """CISO v1: 24-byte header, (frames + 1) u32 index (bit 31 = stored plain, align 0),
    frames raw-deflated unless that does not shrink them."""
    frames = (len(iso) + block - 1) // block
    header = b"CISO" + struct.pack("<IQIBBH", 0x18, len(iso), block, 1, 0, 0)
    body, index = bytearray(), []
    data_start = len(header) + 4 * (frames + 1)
    for n in range(frames):
        raw = iso[n * block:(n + 1) * block].ljust(block, b"\0")
        packer = zlib.compressobj(9, zlib.DEFLATED, -15)
        packed = packer.compress(raw) + packer.flush()
        offset = data_start + len(body)
        if len(packed) < len(raw):
            index.append(offset)
            body += packed
        else:
            index.append(offset | 0x80000000)
            body += raw
    index.append(data_start + len(body))
    return header + b"".join(struct.pack("<I", v) for v in index) + bytes(body)


def eboot_pbp(sfo, data_psp):
    """Homebrew EBOOT.PBP: PARAM.SFO, no icons/pictures/sound, DATA.PSP = the ELF,
    empty DATA.PSAR (offset = file size)."""
    sfo_at = 0x28
    data_at = sfo_at + len(sfo)
    data_at += -data_at % 16
    end = data_at + len(data_psp)
    offsets = [sfo_at] + [data_at] * 5 + [data_at, end]
    head = b"\0PBP" + struct.pack("<I", 0x00010000) + struct.pack("<8I", *offsets)
    return (head + sfo).ljust(data_at, b"\0") + data_psp


def game_sfo(category, disc_id, title):
    """PARAM.SFO of a single-disc game or eboot (`category` UG / MG)."""
    return param_sfo({
        "BOOTABLE": 1, "CATEGORY": category, "DISC_ID": disc_id, "DISC_NUMBER": 1,
        "DISC_TOTAL": 1, "DISC_VERSION": "1.00", "PARENTAL_LEVEL": 1,
        "PSP_SYSTEM_VER": "1.00", "REGION": 32768, "TITLE": title,
    })


def umd_iso(serial, sfo, boot):
    """UMD image: UMD_DATA.BIN (`serial|…`), PSP_GAME/PARAM.SFO, SYSDIR/EBOOT.BIN."""
    return iso_image({
        "UMD_DATA.BIN": f"{serial}|0000000000000001|0001|G".encode(),
        "PSP_GAME": {"PARAM.SFO": sfo, "SYSDIR": {"EBOOT.BIN": boot}},
    })
