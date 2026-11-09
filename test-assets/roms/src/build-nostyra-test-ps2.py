#!/usr/bin/env python3
"""Builds the Nostyra PlayStation 2 test content: self-written, GPLv3, no PS2SDK.

Same approach as the other test ROM builders: Python 3 only, byte-for-byte
reproducible output, hand-assembled EE code (MIPS I subset on the Allegrex emitter,
mnemonics in the comments). PCEE2 boots it through the user's own BIOS (fast boot);
there is no HLE BIOS, so the real-core tests run only where a dump exists.

What the program does (ps2_test_elf.py; each feature backs a runner test on PCEE2):
- Video: SetGsCrt NTSC + GS privileged registers, then every VSINT a full-screen
  sprite is DMA'd to the GIF with a colour that ramps per frame (non-black picture,
  consecutive frames differ). No input, no audio (IOP modules are out of scope).
- RAM: 'NSTY' + the frame counter at EE RAM 0x00108000 (RetroAchievements: EE RAM is
  RA address 0 -> the counter byte is $108004).

Outputs (next to this script's parent folder): nostyra-test-ps2.elf and the
DVD-style ISO nostyra-test-ps2.iso: PVD system id PLAYSTATION, volume
NOSTYRA_TEST_PS2, root SYSTEM.CNF `BOOT2 = cdrom0:\\NSTR_000.01;1` (serial
NSTR-00001), the ELF as NSTR_000.01, L + M path tables (the PS2 kernel's cdvd driver
walks them) and a 2 MiB padding (the kernel's read-ahead past the image end
otherwise keeps the ELF from starting; M18 spike). The CHD / CSO / ZSO twins are
built in test code from the ISO.

Usage: python build-nostyra-test-ps2.py [output folder]
"""
import hashlib
import pathlib
import struct
import sys

import ps2_test_elf
from psp_test_disc import iso_image

SYSTEM_CNF = b"BOOT2 = cdrom0:\\NSTR_000.01;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n"
SECTOR = 2048
NUL = bytes(1)
IMAGE_BYTES = 2 << 20


def with_path_tables(iso):
    """Appends L and M path tables (root only), pads to IMAGE_BYTES and points the
    PVD at them."""
    sectors = len(iso) // SECTOR
    total = IMAGE_BYTES // SECTOR
    pvd_at = 16 * SECTOR
    root_lba = int.from_bytes(iso[pvd_at + 158:pvd_at + 162], "little")
    rec_l = bytes([1, 0]) + struct.pack("<I", root_lba) + struct.pack("<H", 1) + NUL * 2
    rec_m = bytes([1, 0]) + struct.pack(">I", root_lba) + struct.pack(">H", 1) + NUL * 2
    pvd = bytearray(iso[pvd_at:pvd_at + SECTOR])
    pvd[80:88] = struct.pack("<I", total) + struct.pack(">I", total)
    pvd[132:140] = struct.pack("<I", len(rec_l)) + struct.pack(">I", len(rec_l))
    pvd[140:144] = struct.pack("<I", sectors)
    pvd[148:152] = struct.pack(">I", sectors + 1)
    out = bytearray(iso)
    out[pvd_at:pvd_at + SECTOR] = pvd
    out += rec_l.ljust(SECTOR, NUL) + rec_m.ljust(SECTOR, NUL)
    return bytes(out).ljust(IMAGE_BYTES, NUL)


def main():
    out_dir = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else
                           pathlib.Path(__file__).resolve().parent.parent)
    elf = ps2_test_elf.elf(ps2_test_elf.program())
    tree = {"SYSTEM.CNF": SYSTEM_CNF, "NSTR_000.01": elf}
    iso = with_path_tables(iso_image(tree, system_id="PLAYSTATION",
                                     volume_id="NOSTYRA_TEST_PS2"))
    for name, data in {"nostyra-test-ps2.elf": elf, "nostyra-test-ps2.iso": iso}.items():
        (out_dir / name).write_bytes(data)
        print(f"{hashlib.sha1(data).hexdigest()}  {name}")
    ra_hash = hashlib.md5(b"NSTR_000.01" + elf).hexdigest()
    print(f"rcheevos ps2 hash md5('NSTR_000.01' + elf) = {ra_hash}")


if __name__ == "__main__":
    main()
