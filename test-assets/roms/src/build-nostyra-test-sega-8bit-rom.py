#!/usr/bin/env python3
"""Builds nostyra-test.sms / .gg / .sg: tiny, self-written, GPLv3 Sega 8-bit test ROMs.

No external toolchain, byte-for-byte reproducible, hand-assembled Z80 with mnemonics in
comments. One program, three targets; only the video and cart-RAM code differ (Genesis
Plus GX picks the system by file extension, so each ROM keeps its own).

What the ROMs do (each feature backs a runner test on Genesis Plus GX):
- Video: a colour that changes every frame (consecutive frames always differ).
  SMS / GG (mode 4): written to CRAM entry 0 (every pixel of the all-zero name table) and
  entry 16 (backdrop, register 7 = 0). SG-1000 (TMS9918 Graphics I): register 7 backdrop,
  colours 8..15 only (colour 0 is transparent and shows like colour 1, black).
- Input: holding Up/Down/Left/Right/1/2 (port $DC, active low) inverts the colour.
- Audio: PSG channel 0 plays a constant ~440 Hz square wave at full volume.
- RAM: the frame counter (+1 per frame, 8 bits) at $C000 (RetroAchievements 0x0000).
- Cart RAM (SMS / GG only): mapper $FFFC bit 3 maps battery RAM at $8000: boot counter
  at $8000, marker $A5 at $8001 (fresh cart initialised). SG-1000: no battery RAM.

Format: SMS / GG 64 KiB (Sega mapper) with a cosmetic `TMR SEGA` header, SG-1000 32 KiB.

Usage: python build-nostyra-test-sega-8bit-rom.py [--target sms|gg|sg] [output dir]
"""
import hashlib
import pathlib
import struct
import sys

TARGETS = {"sms": 0x10000, "gg": 0x10000, "sg": 0x8000}  # target -> ROM size
VDP_DATA, VDP_CTRL, PAD, PSG = 0xBE, 0xBF, 0xDC, 0x7F
MAPPER_RAM_CTRL, BOOT_COUNTER, MARKER = 0xFFFC, 0x8000, 0x8001
STACK_TOP = 0xDFF0
FRAME_COUNTER = 0xC000  # first byte of system RAM: RetroAchievements address 0x0000
# PSG tone: 3579545 Hz / (32 * 440) = 254 -> latch low 4 bits, then high 6 bits.
PSG_TONE = 254
# VDP registers (value, register). Mode 4 for SMS / GG: display on, no interrupts,
# name table $3800, backdrop = sprite palette colour 0 (CRAM 16). SG-1000 Graphics I:
# display on (16 KiB), name $3800, colour $2000, patterns $0000, sprites off-screen.
VDP_REGS = {
    "sms": [(0x04, 0), (0xC0, 1), (0xFF, 2), (0xFF, 3), (0xFF, 4), (0xFF, 5), (0xFB, 6),
            (0x00, 7), (0x00, 8), (0x00, 9), (0xFF, 10)],
    "sg": [(0x00, 0), (0xC0, 1), (0x0E, 2), (0x80, 3), (0x00, 4), (0x76, 5), (0x03, 6),
           (0x00, 7)],
}
VDP_REGS["gg"] = VDP_REGS["sms"]
# CRAM write commands (low byte first): entry 0 and entry 16 (SMS 1 byte / entry, GG 2).
CRAM_ENTRIES = {"sms": [0x00, 0x10], "gg": [0x00, 0x20]}


class Asm:
    """Minimal byte emitter with labels, 8-bit relative jumps and absolute jumps."""

    def __init__(self, origin=0):
        self.origin = origin
        self.buf = bytearray()
        self.labels = {}
        self.fixups = []  # (offset, label, kind) with kind "rel" or "abs"

    @property
    def pc(self):
        return self.origin + len(self.buf)

    def label(self, name):
        self.labels[name] = self.pc

    def emit(self, *data):
        self.buf.extend(data)

    def org(self, addr):
        assert addr >= self.pc, f"org {addr:#x} overlaps code at {self.pc:#x}"
        self.buf.extend(bytes(addr - self.pc))

    def rel(self, opcode, name):
        self.emit(opcode, 0)
        self.fixups.append((len(self.buf) - 1, name, "rel"))

    def abs(self, opcode, name):
        self.emit(opcode, 0, 0)
        self.fixups.append((len(self.buf) - 2, name, "abs"))

    def resolve(self):
        for off, name, kind in self.fixups:
            target = self.labels[name]
            if kind == "abs":
                self.buf[off:off + 2] = struct.pack("<H", target)
                continue
            rel = target - (self.origin + off + 1)
            if not -128 <= rel <= 127:
                raise ValueError(f"jump to {name} out of range")
            self.buf[off] = rel & 0xFF
        return bytes(self.buf)


def le16(v):
    return v & 0xFF, (v >> 8) & 0xFF


def build_program(target):
    a = Asm()
    a.emit(0xF3)                                  # di
    a.emit(0xED, 0x56)                            # im 1
    a.emit(0x31, *le16(STACK_TOP))                # ld sp,$DFF0
    a.abs(0xC3, "main")                           # jp main
    a.org(0x38)
    a.emit(0xFB)                                  # ei        (IM 1 handler; never enabled)
    a.emit(0xED, 0x4D)                            # reti
    a.org(0x66)
    a.emit(0xED, 0x45)                            # retn      (SMS Pause = NMI: ignored)

    a.label("main")
    if target != "sg":
        a.emit(0x3E, 0x08)                        # ld a,$08
        a.emit(0x32, *le16(MAPPER_RAM_CTRL))      # ld ($FFFC),a (cart RAM at $8000)
        a.emit(0x3A, *le16(MARKER))               # ld a,($8001)
        a.emit(0xFE, 0xA5)                        # cp $A5
        a.rel(0x28, "ram_ready")                  # jr z,ram_ready
        a.emit(0xAF)                              # xor a
        a.emit(0x32, *le16(BOOT_COUNTER))         # ld ($8000),a (fresh cart)
        a.emit(0x3E, 0xA5)                        # ld a,$A5
        a.emit(0x32, *le16(MARKER))               # ld ($8001),a
        a.label("ram_ready")
        a.emit(0x21, *le16(BOOT_COUNTER))         # ld hl,$8000
        a.emit(0x34)                              # inc (hl)  (boot counter)
    a.emit(0xDB, VDP_CTRL)                        # in a,($BF) (reset the control latch)
    for value, reg in VDP_REGS[target]:
        a.emit(0x3E, value)                       # ld a,value
        a.emit(0xD3, VDP_CTRL)                    # out ($BF),a
        a.emit(0x3E, 0x80 | reg)                  # ld a,$80|reg
        a.emit(0xD3, VDP_CTRL)                    # out ($BF),a
    for value in (0x80 | (PSG_TONE & 0x0F), PSG_TONE >> 4, 0x90):
        a.emit(0x3E, value)                       # ld a,value (tone latch, tone data, volume 0)
        a.emit(0xD3, PSG)                         # out ($7F),a
    a.emit(0x1E, 0x00)                            # ld e,0     (colour counter)

    a.label("wait_vblank")
    a.emit(0xDB, VDP_CTRL)                        # in a,($BF) (status; read clears the flag)
    a.emit(0xE6, 0x80)                            # and $80   (frame interrupt flag)
    a.rel(0x28, "wait_vblank")                    # jr z,wait_vblank
    a.emit(0x1C)                                  # inc e
    a.emit(0x7B)                                  # ld a,e
    a.emit(0x32, *le16(FRAME_COUNTER))            # ld ($C000),a (frame counter in RAM)
    if target == "sg":
        a.emit(0xE6, 0x07)                        # and $07
        a.emit(0xF6, 0x08)                        # or $08    (colours 8..15: 0 is transparent
        #                                                       and would look like 1 = black)
    else:
        a.emit(0xE6, 0x3F)                        # and $3F   (64 colours)
    a.emit(0x57)                                  # ld d,a
    a.emit(0xDB, PAD)                             # in a,($DC)
    a.emit(0xE6, 0x3F)                            # and $3F
    a.emit(0xFE, 0x3F)                            # cp $3F    (all released = $3F)
    a.emit(0x7A)                                  # ld a,d    (flags kept)
    a.rel(0x28, "write_colour")                   # jr z,write_colour
    a.emit(0xEE, 0x07 if target == "sg" else 0x3F)  # xor mask (pressed: another colour)
    a.label("write_colour")
    if target == "sg":
        a.emit(0xD3, VDP_CTRL)                    # out ($BF),a (register 7 value)
        a.emit(0x3E, 0x87)                        # ld a,$87
        a.emit(0xD3, VDP_CTRL)                    # out ($BF),a
    else:
        a.emit(0x57)                              # ld d,a
        for entry in CRAM_ENTRIES[target]:
            a.emit(0x3E, entry)                   # ld a,entry (CRAM address low)
            a.emit(0xD3, VDP_CTRL)                # out ($BF),a
            a.emit(0x3E, 0xC0)                    # ld a,$C0   (CRAM write)
            a.emit(0xD3, VDP_CTRL)                # out ($BF),a
            a.emit(0x7A)                          # ld a,d
            a.emit(0xD3, VDP_DATA)                # out ($BE),a (SMS: --BBGGRR; GG: GGGGRRRR)
            if target == "gg":
                a.emit(0xD3, VDP_DATA)            # out ($BE),a (GG high byte: ----BBBB)
    a.abs(0xC3, "wait_vblank")                    # jp wait_vblank
    return a.resolve()


def build_rom(target):
    size = TARGETS[target]
    rom = bytearray(size)
    code = build_program(target)
    rom[:len(code)] = code
    if target != "sg":
        # TMR SEGA header: checksum of $0000-$7FEF, product 0, version 0, region + size
        # nibbles (SMS export 0x4 / GG international 0x7; size 0xC = 32 KiB checked).
        checksum = sum(rom[:0x7FF0]) & 0xFFFF
        region = 0x4 if target == "sms" else 0x7
        rom[0x7FF0:0x8000] = (b"TMR SEGA" + bytes(2) + struct.pack("<H", checksum)
                              + bytes(3) + bytes([region << 4 | 0xC]))
    return bytes(rom)


def main():
    args = sys.argv[1:]
    targets = list(TARGETS)
    if args[:1] == ["--target"]:
        targets, args = [args[1]], args[2:]
    out_dir = pathlib.Path(args[0]) if args else pathlib.Path(__file__).resolve().parent.parent
    for target in targets:
        rom = build_rom(target)
        out = out_dir / f"nostyra-test.{target}"
        out.write_bytes(rom)
        print(f"wrote {out} ({len(rom)} bytes, sha1 {hashlib.sha1(rom).hexdigest()})")


if __name__ == "__main__":
    main()
