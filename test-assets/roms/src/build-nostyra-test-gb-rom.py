#!/usr/bin/env python3
"""Builds nostyra-test.gb and nostyra-test.gbc: tiny, self-written, GPLv3 Game Boy
test ROMs (one program; the CGB flag at 0x143 and the cartridge type differ).

Same approach as build-nostyra-test-nes-rom.py: no external toolchain, output
byte-for-byte reproducible, hand-assembled SM83 opcodes with mnemonics in comments.

What the ROMs do (each feature backs a runner test on the mgba core):
- Video: the background colour changes every frame. DMG: BGP ($FF47) = frame
  counter. CGB: BG palette 0 colour 0 (BCPS/BCPD) low byte = frame counter.
  Consecutive frames always differ.
- Input: holding any joypad button switches palette (DMG: BGP inverted; CGB:
  colour high byte $7C instead of $00).
- Audio: channel 1 plays a constant ~440 Hz square wave.
- SRAM: 8 KiB battery RAM; byte $A000 is a boot counter (+1 per reset), byte $A001
  a marker ($A5). Fresh cart RAM reads $FF, so the first boot
  without the marker clears the counter first (first boot = 1). Nothing else is
  written, so a reset changes only byte 0.
- Clock: nostyra-test.gb is MBC3+TIMER+RAM+BATTERY and latches the MBC3 clock every
  frame (writes 0 then 1 to $6000, never reads it), so mGBA keeps the clock current
  in RETRO_MEMORY_RTC (the runner's rtc.bin). nostyra-test.gbc is MBC1+RAM+BATTERY
  (no clock; there the $6000 writes only select the MBC1 banking mode, a no-op for
  a 32 KiB ROM with one RAM bank).

Header: entry `nop; jp $0150`, only the first 4 Nintendo logo bytes at 0x104 (what
mGBA checks; the rest of the logo is zero, so a real boot ROM rejects the cart),
title "NOSTYRA TEST", 32 KiB ROM, 8 KiB RAM, header and global checksums.

Usage: python build-nostyra-test-gb-rom.py [output dir]
"""
import hashlib
import pathlib
import sys

ROM_SIZE = 0x8000
ENTRY = 0x0150
# I/O registers (offsets from $FF00 for ldh).
P1, NR11, NR12, NR13, NR14, NR50, NR51, NR52 = 0x00, 0x11, 0x12, 0x13, 0x14, 0x24, 0x25, 0x26
LCDC, LY, BGP, BCPS, BCPD = 0x40, 0x44, 0x47, 0x68, 0x69
FRAME = 0x80  # HRAM $FF80: frame counter
RAM_ENABLE, CLOCK_LATCH, BOOT_COUNTER, MARKER = 0x0000, 0x6000, 0xA000, 0xA001
MBC1_RAM_BATTERY, MBC3_TIMER_RAM_BATTERY = 0x03, 0x10
VBLANK_LINE = 144
# Channel 1 frequency for ~440 Hz: 2048 - 131072 / 440 = 1750 ($6D6).
SQUARE_FREQ = 1750


class Asm:
    """Minimal byte emitter with labels, 8-bit relative jumps and absolute jumps."""

    def __init__(self, origin):
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

    def jr(self, opcode, name):
        self.emit(opcode, 0)
        self.fixups.append((len(self.buf) - 1, name, "rel"))

    def jp(self, name):
        self.emit(0xC3, 0, 0)
        self.fixups.append((len(self.buf) - 2, name, "abs"))

    def resolve(self):
        for off, name, kind in self.fixups:
            target = self.labels[name]
            if kind == "abs":
                self.buf[off:off + 2] = bytes([lo(target), hi(target)])
                continue
            rel = target - (self.origin + off + 1)
            if not -128 <= rel <= 127:
                raise ValueError(f"jump to {name} out of range")
            self.buf[off] = rel & 0xFF
        return bytes(self.buf)


def lo(v):
    return v & 0xFF


def hi(v):
    return (v >> 8) & 0xFF


JR, JR_NZ, JR_Z = 0x18, 0x20, 0x28


def ld_a(a, value):
    a.emit(0x3E, value)               # LD A,#value


def ldh_store(a, reg):
    a.emit(0xE0, reg)                 # LDH ($FF00+reg),A


def ldh_load(a, reg):
    a.emit(0xF0, reg)                 # LDH A,($FF00+reg)


def ld_store(a, addr):
    a.emit(0xEA, lo(addr), hi(addr))  # LD (addr),A


def ld_load(a, addr):
    a.emit(0xFA, lo(addr), hi(addr))  # LD A,(addr)


def build_program():
    a = Asm(ENTRY)
    a.emit(0xF3)                      # DI
    a.emit(0x31, 0xFE, 0xFF)          # LD SP,$FFFE
    # APU: master on first, then volume/panning, then channel 1 + trigger.
    for reg, value in ((NR52, 0x80), (NR50, 0x77), (NR51, 0x11), (NR11, 0x80),
                       (NR12, 0xF0), (NR13, lo(SQUARE_FREQ)), (NR14, 0x80 | hi(SQUARE_FREQ))):
        ld_a(a, value)
        ldh_store(a, reg)
    # SRAM: enable, init counter on a fresh cart, bump the boot counter, disable.
    ld_a(a, 0x0A)
    ld_store(a, RAM_ENABLE)
    ld_load(a, MARKER)
    a.emit(0xFE, 0xA5)                # CP $A5
    a.jr(JR_Z, "marked")
    a.emit(0xAF)                      # XOR A
    ld_store(a, BOOT_COUNTER)
    ld_a(a, 0xA5)
    ld_store(a, MARKER)
    a.label("marked")
    ld_load(a, BOOT_COUNTER)
    a.emit(0x3C)                      # INC A
    ld_store(a, BOOT_COUNTER)
    a.emit(0xAF)                      # XOR A
    ld_store(a, RAM_ENABLE)           # RAM off
    ldh_store(a, FRAME)               # frame = 0
    ld_a(a, 0x91)
    ldh_store(a, LCDC)                # LCD on, BG on

    a.label("loop")
    a.label("wait_vblank")
    ldh_load(a, LY)
    a.emit(0xFE, VBLANK_LINE)         # CP 144
    a.jr(JR_NZ, "wait_vblank")
    # Joypad: select both button groups, read the low nibble ($0F = nothing held).
    a.emit(0xAF)                      # XOR A
    ldh_store(a, P1)
    for _ in range(4):
        ldh_load(a, P1)               # (repeated reads let the lines settle)
    a.emit(0xE6, 0x0F)                # AND $0F
    a.emit(0x47)                      # LD B,A
    ld_a(a, 0x30)
    ldh_store(a, P1)                  # deselect
    ldh_load(a, FRAME)
    a.emit(0x3C)                      # INC A
    ldh_store(a, FRAME)
    a.emit(0x4F)                      # LD C,A
    # DMG palette: frame counter, inverted while a button is held.
    a.emit(0x78)                      # LD A,B
    a.emit(0xFE, 0x0F)                # CP $0F
    a.emit(0x79)                      # LD A,C
    a.jr(JR_Z, "dmg_plain")
    a.emit(0x2F)                      # CPL
    a.label("dmg_plain")
    ldh_store(a, BGP)
    # CGB palette 0 colour 0: low byte = frame counter, high byte $00 or $7C (held).
    ld_a(a, 0x80)
    ldh_store(a, BCPS)                # index 0, auto-increment
    a.emit(0x79)                      # LD A,C
    ldh_store(a, BCPD)
    a.emit(0x78)                      # LD A,B
    a.emit(0xFE, 0x0F)                # CP $0F
    ld_a(a, 0x00)
    a.jr(JR_Z, "cgb_plain")
    ld_a(a, 0x7C)
    a.label("cgb_plain")
    ldh_store(a, BCPD)
    # MBC3 clock latch (0 -> 1); MBC1: banking mode select, harmless here.
    a.emit(0xAF)                      # XOR A
    ld_store(a, CLOCK_LATCH)
    a.emit(0x3C)                      # INC A
    ld_store(a, CLOCK_LATCH)
    a.label("wait_line")
    ldh_load(a, LY)
    a.emit(0xFE, VBLANK_LINE)         # CP 144
    a.jr(JR_Z, "wait_line")
    a.jr(JR, "loop")
    return a.resolve()


def build_rom(cgb_flag, cart_type):
    rom = bytearray(ROM_SIZE)
    rom[0x100:0x104] = bytes([0x00, 0xC3, lo(ENTRY), hi(ENTRY)])  # NOP; JP $0150
    rom[0x104:0x108] = bytes([0xCE, 0xED, 0x66, 0x66])
    rom[0x134:0x140] = b"NOSTYRA TEST"
    rom[0x143] = cgb_flag
    rom[0x146] = 0x00  # no SGB functions
    rom[0x147] = cart_type
    rom[0x148] = 0x00  # 32 KiB ROM
    rom[0x149] = 0x02  # 8 KiB RAM
    rom[0x14A] = 0x01  # non-Japanese
    check = 0
    for byte in rom[0x134:0x14D]:
        check = (check - byte - 1) & 0xFF
    rom[0x14D] = check
    code = build_program()
    rom[ENTRY:ENTRY + len(code)] = code
    total = sum(rom) & 0xFFFF  # global checksum bytes are still zero here
    rom[0x14E:0x150] = bytes([hi(total), lo(total)])
    return bytes(rom)


def main():
    out_dir = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else (
        pathlib.Path(__file__).resolve().parent.parent)
    for name, cgb_flag, cart_type in (("nostyra-test.gb", 0x00, MBC3_TIMER_RAM_BATTERY),
                                      ("nostyra-test.gbc", 0x80, MBC1_RAM_BATTERY)):
        rom = build_rom(cgb_flag, cart_type)
        out = out_dir / name
        out.write_bytes(rom)
        print(f"wrote {out} ({len(rom)} bytes, sha1 {hashlib.sha1(rom).hexdigest()})")


if __name__ == "__main__":
    main()
