#!/usr/bin/env python3
"""Builds two tiny, self-written, GPLv3 Nintendo DS test ROMs (melonDS DS, direct boot).

Same approach as the GBA builder (hand-assembled 32-bit ARM from arm_emitter.py,
byte-for-byte reproducible output, no toolchain). The programs live in
nds_test_arm9.py (picture, keys, touch tint, 3D triangle) and nds_test_arm7.py
(tone, touch-screen controller, EEPROM boot counter). No Nintendo logo bytes: the
logo area stays zero.

ROM A `nostyra-test.nds` (homebrew: ARM9 at ROM offset 0x200 -> 0x02000000, game
code ZNSA): top screen red (green while a key is held), bottom screen blue, frame
bits in both; while the pen is down the bottom screen shows the touch position
(red = x / 8, green = y / 8); PSG tone.
ROM B `nostyra-test-save.nds` (retail-like: ARM9 at 0x4000 = decrypted secure-area
layout, first words 0xE7FFDEFF x2, entry RAM+0x20, logo-CRC field 0xCF56 with a
zero logo, game code ZNSB not in melonDS's ROM list -> 8 KiB EEPROM): the same
program plus the ARM7 boot counter in EEPROM bytes 0 (counter) / 1 (0xA5 marker).

Usage: python build-nostyra-test-nds.py [out-dir]
"""
import hashlib
import pathlib
import struct
import sys

from nds_test_arm7 import ARM7_RAM, arm7_program
from nds_test_arm9 import ARM9_RAM, arm9_program

MARKER = 0xE7FFDEFF       # decrypted secure-area marker (melonDS: "romDecrypted")
ROM_A, ROM_B = "nostyra-test.nds", "nostyra-test-save.nds"


def crc16(data):
    """CRC-16/MODBUS, the DS header and logo checksum."""
    crc = 0xFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc


def build_rom(retail):
    """ROM B when `retail`, else ROM A."""
    arm7 = arm7_program(retail)
    if retail:
        arm9_off, header_size, size, code = 0x4000, 0x4000, 0x8000, b"ZNSB"
        arm9 = struct.pack("<II", MARKER, MARKER) + bytes(0x18) + arm9_program(ARM9_RAM + 0x20)
        entry = ARM9_RAM + 0x20
        arm7_off = 0x6000
    else:
        arm9_off, header_size, size, code = 0x200, 0x200, 0x2000, b"ZNSA"
        arm9 = arm9_program()
        entry = ARM9_RAM
        arm7_off = 0x1000
    assert arm9_off + len(arm9) <= arm7_off and arm7_off + len(arm7) <= size
    rom = bytearray(size)
    rom[0:12] = b"NOSTYRA TEST"
    rom[0x0C:0x10] = code
    rom[0x10:0x12] = b"00"
    struct.pack_into("<IIII", rom, 0x20, arm9_off, entry, ARM9_RAM, len(arm9))
    struct.pack_into("<IIII", rom, 0x30, arm7_off, ARM7_RAM, ARM7_RAM, len(arm7))
    struct.pack_into("<II", rom, 0x80, size, header_size)
    logo_crc = 0xCF56 if retail else crc16(bytes(0x9C))
    struct.pack_into("<H", rom, 0x15C, logo_crc)
    struct.pack_into("<H", rom, 0x15E, crc16(rom[0:0x15E]))
    rom[arm9_off:arm9_off + len(arm9)] = arm9
    rom[arm7_off:arm7_off + len(arm7)] = arm7
    return bytes(rom)


def main():
    default_out = pathlib.Path(__file__).resolve().parent.parent
    out_dir = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_out
    for name, rom in ((ROM_A, build_rom(False)), (ROM_B, build_rom(True))):
        (out_dir / name).write_bytes(rom)
        print(f"wrote {out_dir / name} ({len(rom)} bytes, sha1 {hashlib.sha1(rom).hexdigest()})")


if __name__ == "__main__":
    main()
