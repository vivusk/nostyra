#!/usr/bin/env python3
"""Builds the self-written Atari test ROMs (GPLv3, like the other test ROMs; no
external toolchain, byte-for-byte reproducible). Programs: atari_test_programs.py.

Outputs (beside test-assets/roms/ by default):
- nostyra-test.a26       Atari 2600, 4 KiB at $F000 (no bankswitch).
- nostyra-test.a78       Atari 7800, 128-byte A78 header + 16 KiB at $C000.
- nostyra-test-7800.bin  the same 16 KiB without the header (headerless dump form).
- nostyra-test.o         Atari Lynx BS93 homebrew (10-byte header, load $0200).
- nostyra-test.lnx       Atari Lynx cartridge: 64-byte LNX header (rotated left,
                         93C46 EEPROM) + 64 KiB bank 0 booted by the cc65 loader
                         (lynx_boot_loader.py, zlib).

Usage: python build-nostyra-test-atari-roms.py [output dir]
"""
import hashlib
import pathlib
import sys

from atari_test_programs import atari2600_program, atari7800_program, lynx_program
from lynx_boot_loader import ENCRYPTED_LOADER, SECOND_STAGE_ADDR, SECOND_STAGE_SIZE
from mos6502_emitter import lo, hi

A26_SIZE, A26_ORIGIN = 0x1000, 0xF000
A78_SIZE, A78_ORIGIN = 0x4000, 0xC000
LYNX_LOAD = 0x0200
LNX_BANK0 = 0x10000  # 64 KiB: 256 pages of 256 bytes
LNX_ROTATE_LEFT, LNX_EEPROM_93C46 = 1, 1


def vectors(nmi, reset, irq):
    return b"".join(bytes([lo(v), hi(v)]) for v in (nmi, reset, irq))


def build_a26():
    code, labels = atari2600_program(A26_ORIGIN)
    rom = bytearray(A26_SIZE)
    rom[:len(code)] = code
    reset = labels["reset"]
    rom[-6:] = vectors(reset, reset, reset)
    return bytes(rom)


def build_7800_image():
    code, labels = atari7800_program(A78_ORIGIN)
    rom = bytearray(b"\xFF" * A78_SIZE)
    rom[:len(code)] = code
    # $FFF8 = $FF, $FFF9 = $C7: the BIOS signature-area bytes (unused without a BIOS).
    rom[-8:-6] = bytes([0xFF, 0xC7])
    rom[-6:] = vectors(labels["nmi"], labels["reset"], labels["nmi"])
    return bytes(rom)


def a78_header(size):
    header = bytearray(128)
    header[0] = 1                                  # header version
    header[1:10] = b"ATARI7800"
    title = b"Nostyra test ROM"
    header[17:17 + len(title)] = title
    header[49:53] = size.to_bytes(4, "big")        # ROM size without the header
    header[55], header[56] = 1, 1                  # controllers: joysticks
    header[57] = 0                                 # NTSC
    header[100:128] = b"ACTUAL CART DATA STARTS HERE"
    return bytes(header)


def build_lynx_o():
    code, _labels = lynx_program(LYNX_LOAD)
    size = 10 + len(code)
    # BRA +8 (over the header), load address and file size big-endian, "BS93".
    header = bytes([0x80, 0x08, hi(LYNX_LOAD), lo(LYNX_LOAD), hi(size), lo(size)]) + b"BS93"
    return header + code


def lnx_header():
    header = bytearray(64)
    header[0:4] = b"LYNX"
    header[4:6] = (LNX_BANK0 // 256).to_bytes(2, "little")  # bank-0 page size
    header[8:10] = (1).to_bytes(2, "little")                 # header version
    title = b"Nostyra test cartridge"
    header[10:10 + len(title)] = title
    header[42:49] = b"Nostyra"
    header[58] = LNX_ROTATE_LEFT
    header[60] = LNX_EEPROM_93C46
    return bytes(header)


def build_lnx():
    code, _labels = lynx_program(SECOND_STAGE_ADDR)
    if len(code) > SECOND_STAGE_SIZE:
        raise ValueError(f"Lynx program is {len(code)} bytes, the loader copies {SECOND_STAGE_SIZE}")
    second = code + bytes(SECOND_STAGE_SIZE - len(code))
    bank0 = bytearray(b"\xFF" * LNX_BANK0)
    image = ENCRYPTED_LOADER + second
    bank0[:len(image)] = image
    return lnx_header() + bytes(bank0)


def main():
    default_out = pathlib.Path(__file__).resolve().parent.parent
    out_dir = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_out
    image_7800 = build_7800_image()
    outputs = {
        "nostyra-test.a26": build_a26(),
        "nostyra-test.a78": a78_header(len(image_7800)) + image_7800,
        "nostyra-test-7800.bin": image_7800,
        "nostyra-test.o": build_lynx_o(),
        "nostyra-test.lnx": build_lnx(),
    }
    for name, data in outputs.items():
        path = out_dir / name
        path.write_bytes(data)
        print(f"wrote {path} ({len(data)} bytes, sha1 {hashlib.sha1(data).hexdigest()})")


if __name__ == "__main__":
    main()
