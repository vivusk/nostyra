#!/usr/bin/env python3
"""Builds nostyra-test.gba: a tiny, self-written, GPLv3 Game Boy Advance test ROM.

Same approach as build-nostyra-test-nes-rom.py: no external toolchain, output
byte-for-byte reproducible, hand-assembled 32-bit ARM instructions with mnemonics
in comments (emitter shared with the DS builder: arm_emitter.py).

What the ROM does (each feature backs a runner test on the mgba core):
- Video: mode 0 with no layers shows the backdrop colour (palette RAM 0x05000000),
  which takes the frame counter every VBlank (consecutive frames always differ).
- Input: holding any key (KEYINPUT != 0x3FF) sets the blue bits (0x7C00).
- RAM: the frame counter (+1 per VBlank, 8 bits) at IWRAM 0x03000000
  (RetroAchievements address 0x000000).
- Audio: DMG channel 1 plays a constant ~440 Hz square wave at full PSG volume.
- SRAM: battery SRAM at 0x0E000000 (8-bit accesses only); byte 0 is a boot counter
  (+1 per reset), byte 1 a marker (0xA5). Fresh SRAM reads 0xFF, so the first boot
  without the marker clears the counter first (first boot = 1). Nothing else is
  written. mGBA picks the save type on the first SRAM access (no override-table
  entry for this game code); the `SRAM_V113` ID string is the usual convention.

Header: `b start` at 0 (byte 3 = 0xEA, what mGBA checks), Nintendo logo area left
zero (a real BIOS rejects the cart), title "NOSTYRA TEST", game code "ZNOS", fixed
byte 0xB2 = 0x96, header complement checksum at 0xBD.

Usage: python build-nostyra-test-gba-rom.py [output.gba]
"""
import hashlib
import pathlib
import struct
import sys

from arm_emitter import (COND_AL, COND_EQ, COND_NE, Arm, add_imm, cmp_imm, ldrb, ldrh, mov,
                         strb, strh)

ROM_SIZE = 0x10000
ROM_BASE = 0x08000000
CODE_OFFSET = 0xC0
IO, PALETTE, SRAM = 0x04000000, 0x05000000, 0x0E000000
IWRAM_COUNTER = 0x03000000  # frame counter byte (RetroAchievements address 0x000000)
DISPCNT, VCOUNT = 0x00, 0x06
SOUND1CNT_L, SOUND1CNT_H, SOUND1CNT_X = 0x60, 0x62, 0x64
SOUNDCNT_L, SOUNDCNT_H, SOUNDCNT_X = 0x80, 0x82, 0x84
KEYINPUT = IO + 0x130
NO_KEYS = 0x3FF
VBLANK_LINE = 160
# Square frequency for ~440 Hz: 2048 - 131072 / 440 = 1750 (0x6D6), bit 15 = restart.
SQUARE_FREQ = 1750
SAVE_ID = b"SRAM_V113"


def build_program():
    a = Arm(ROM_BASE + CODE_OFFSET)
    # SRAM: init on a fresh cart, then bump the boot counter.
    a.ldr_const(0, SRAM)                          # r0 = SRAM
    a.emit(ldrb(1, 0, 1))                         # LDRB r1,[r0,#1]
    a.emit(cmp_imm(1, 0xA5))                      # CMP r1,#0xA5
    a.b(COND_EQ, "marked")                        # BEQ marked
    a.emit(mov(1, 0))                             # MOV r1,#0
    a.emit(strb(1, 0, 0))                         # STRB r1,[r0]
    a.emit(mov(1, 0xA5))                          # MOV r1,#0xA5
    a.emit(strb(1, 0, 1))                         # STRB r1,[r0,#1]
    a.label("marked")
    a.emit(ldrb(1, 0, 0))                         # LDRB r1,[r0]
    a.emit(add_imm(1, 1, 1))                      # ADD r1,r1,#1
    a.emit(strb(1, 0, 0))                         # STRB r1,[r0]
    # Sound: master enable first, then mixer, then channel 1 + restart.
    a.ldr_const(2, IO)                            # r2 = I/O base
    a.emit(mov(3, 0x80))
    a.emit(strh(3, 2, SOUNDCNT_X))                # master on
    a.ldr_const(3, 0x1177)
    a.emit(strh(3, 2, SOUNDCNT_L))                # PSG volume 7/7, ch1 left + right
    a.emit(mov(3, 0x02))
    a.emit(strh(3, 2, SOUNDCNT_H))                # PSG ratio 100 %
    a.emit(mov(3, 0x00))
    a.emit(strh(3, 2, SOUND1CNT_L))               # sweep off
    a.ldr_const(3, 0xF080)
    a.emit(strh(3, 2, SOUND1CNT_H))               # volume 15, no envelope, duty 50 %
    a.ldr_const(3, 0x8000 | SQUARE_FREQ)
    a.emit(strh(3, 2, SOUND1CNT_X))               # frequency + restart
    # Video: mode 0, no layers, forced blank off -> backdrop colour only.
    a.emit(mov(3, 0x00))
    a.emit(strh(3, 2, DISPCNT))
    a.ldr_const(4, PALETTE)                       # r4 = backdrop colour
    a.ldr_const(10, KEYINPUT)                     # r10 = KEYINPUT
    a.ldr_const(8, NO_KEYS)                       # r8 = 0x3FF (nothing held)
    a.emit(mov(6, 0x00))                          # r6 = frame counter
    a.ldr_const(11, IWRAM_COUNTER)                # r11 = its copy in IWRAM

    a.label("loop")
    a.label("wait_vblank")
    a.emit(ldrh(5, 2, VCOUNT))                    # LDRH r5,[r2,#VCOUNT]
    a.emit(cmp_imm(5, VBLANK_LINE))               # CMP r5,#160
    a.b(COND_NE, "wait_vblank")                   # BNE wait_vblank
    a.emit(add_imm(6, 6, 1))                      # ADD r6,r6,#1
    a.emit(strb(6, 11))                           # STRB r6,[r11] (frame counter in IWRAM)
    a.emit(ldrh(7, 10))                           # LDRH r7,[r10]
    a.emit(0xE1570008)                            # CMP r7,r8
    a.emit(0xE1A09006)                            # MOV r9,r6
    a.emit(0x13899C7C)                            # ORRNE r9,r9,#0x7C00
    a.emit(strh(9, 4))                            # STRH r9,[r4]
    a.label("wait_line")
    a.emit(ldrh(5, 2, VCOUNT))
    a.emit(cmp_imm(5, VBLANK_LINE))
    a.b(COND_EQ, "wait_line")                     # BEQ wait_line
    a.b(COND_AL, "loop")                          # B loop
    return a.resolve()


def build_rom():
    rom = bytearray(ROM_SIZE)
    branch = 0xEA000000 | ((CODE_OFFSET - 8) >> 2)  # B start
    rom[0:4] = struct.pack("<I", branch)
    rom[0xA0:0xAC] = b"NOSTYRA TEST"
    rom[0xAC:0xB0] = b"ZNOS"
    rom[0xB0:0xB2] = b"00"
    rom[0xB2] = 0x96
    check = 0
    for byte in rom[0xA0:0xBD]:
        check = (check - byte) & 0xFF
    rom[0xBD] = (check - 0x19) & 0xFF
    code = build_program()
    rom[CODE_OFFSET:CODE_OFFSET + len(code)] = code
    id_at = (CODE_OFFSET + len(code) + 3) & ~3
    rom[id_at:id_at + len(SAVE_ID)] = SAVE_ID
    return bytes(rom)


def main():
    default_out = pathlib.Path(__file__).resolve().parent.parent / "nostyra-test.gba"
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_out
    rom = build_rom()
    out.write_bytes(rom)
    print(f"wrote {out} ({len(rom)} bytes, sha1 {hashlib.sha1(rom).hexdigest()})")


if __name__ == "__main__":
    main()
