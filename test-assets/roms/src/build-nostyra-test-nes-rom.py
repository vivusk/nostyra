#!/usr/bin/env python3
"""Builds nostyra-test.nes: a tiny, self-written, GPLv3 NES test ROM.

Same approach as build-nostyra-test-rom.py (SNES): no external toolchain, output
byte-for-byte reproducible, hand-assembled 6502 opcodes with mnemonics in comments.

What the ROM does (each feature backs a runner test):
- Video: the backdrop colour ($3F00) cycles through 12 hues, one step per frame
  (deterministic frames; consecutive frames always differ).
- Input: holding any joypad-1 button switches to the brighter colour row.
- Audio: APU pulse 1 plays a constant ~440 Hz square wave.
- SRAM: 8 KiB battery PRG-RAM at $6000; byte $6000 is a boot counter (+1 per
  reset), byte $6001 a constant marker ($A5). Nothing else is written, so a
  reset changes only byte 0.

Format: iNES, mapper 0 (NROM-128): 16 KiB PRG mirrored at $8000/$C000, 8 KiB
zero-filled CHR (no art), battery flag, vertical mirroring.

Usage: python build-nostyra-test-nes-rom.py [output.nes]
"""
import hashlib
import pathlib
import sys

PRG_SIZE = 0x4000
CHR_SIZE = 0x2000
ORIGIN = 0xC000

PPUCTRL, PPUMASK, PPUSTATUS, PPUADDR, PPUDATA = 0x2000, 0x2001, 0x2002, 0x2006, 0x2007
SQ1_VOL, SQ1_SWEEP, SQ1_LO, SQ1_HI, SND_CHN, JOY1 = 0x4000, 0x4001, 0x4002, 0x4003, 0x4015, 0x4016
BOOT_COUNTER, MARKER = 0x6000, 0x6001
HUE, PRESSED = 0x01, 0x02  # zero-page variables
# Pulse timer for ~440 Hz: CPU 1.789773 MHz / (16 * 440) - 1 = 253.
PULSE_TIMER = 253


class Asm:
    """Minimal byte emitter with labels, 8-bit relative branches and absolute jumps."""

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

    def abs_op(self, opcode, addr):
        self.emit(opcode, lo(addr), hi(addr))

    def branch(self, opcode, name):
        self.emit(opcode, 0)
        self.fixups.append((len(self.buf) - 1, name, "rel"))

    def jmp(self, name):
        self.emit(0x4C, 0, 0)
        self.fixups.append((len(self.buf) - 2, name, "abs"))

    def resolve(self):
        for off, name, kind in self.fixups:
            target = self.labels[name]
            if kind == "abs":
                self.buf[off:off + 2] = bytes([lo(target), hi(target)])
                continue
            rel = target - (self.origin + off + 1)
            if not -128 <= rel <= 127:
                raise ValueError(f"branch to {name} out of range")
            self.buf[off] = rel & 0xFF
        return bytes(self.buf)


def lo(v):
    return v & 0xFF


def hi(v):
    return (v >> 8) & 0xFF


def build_program():
    a = Asm(ORIGIN)
    a.label("reset")
    a.emit(0x78)                      # SEI
    a.emit(0xD8)                      # CLD
    a.emit(0xA2, 0xFF)                # LDX #$FF
    a.emit(0x9A)                      # TXS
    a.emit(0xA9, 0x00)                # LDA #0
    a.abs_op(0x8D, PPUCTRL)           # STA PPUCTRL   (NMI off)
    a.abs_op(0x8D, PPUMASK)           # STA PPUMASK   (rendering off: backdrop only)
    a.abs_op(0x2C, PPUSTATUS)         # BIT PPUSTATUS (clear vblank flag)
    for wait in ("vblank1", "vblank2"):
        a.label(wait)
        a.abs_op(0x2C, PPUSTATUS)     # BIT PPUSTATUS
        a.branch(0x10, wait)          # BPL wait
    a.abs_op(0xEE, BOOT_COUNTER)      # INC $6000     (boot counter)
    a.emit(0xA9, 0xA5)                # LDA #$A5
    a.abs_op(0x8D, MARKER)            # STA $6001     (constant marker)
    a.emit(0xA9, 0x01)                # LDA #1
    a.abs_op(0x8D, SND_CHN)           # STA $4015     (pulse 1 on)
    a.emit(0xA9, 0xBF)                # LDA #$BF      (duty 50 %, halt, constant volume 15)
    a.abs_op(0x8D, SQ1_VOL)
    a.emit(0xA9, 0x08)                # LDA #$08      (sweep off)
    a.abs_op(0x8D, SQ1_SWEEP)
    a.emit(0xA9, lo(PULSE_TIMER))     # LDA #timer lo
    a.abs_op(0x8D, SQ1_LO)
    a.emit(0xA9, hi(PULSE_TIMER))     # LDA #timer hi (length counter load)
    a.abs_op(0x8D, SQ1_HI)
    a.emit(0xA9, 0x01)                # LDA #1
    a.emit(0x85, HUE)                 # STA hue
    a.emit(0xA9, 0x80)                # LDA #$80
    a.abs_op(0x8D, PPUCTRL)           # STA PPUCTRL   (NMI on)
    a.label("forever")
    a.jmp("forever")                  # JMP forever

    a.label("nmi")
    a.emit(0x48)                      # PHA
    a.abs_op(0xAD, PPUSTATUS)         # LDA PPUSTATUS (ack vblank)
    a.emit(0xE6, HUE)                 # INC hue       (1..12)
    a.emit(0xA5, HUE)                 # LDA hue
    a.emit(0xC9, 13)                  # CMP #13
    a.branch(0xD0, "hue_ok")          # BNE hue_ok
    a.emit(0xA9, 0x01)                # LDA #1
    a.emit(0x85, HUE)                 # STA hue
    a.label("hue_ok")
    a.emit(0xA9, 0x01)                # LDA #1
    a.abs_op(0x8D, JOY1)              # STA $4016     (strobe on)
    a.emit(0xA9, 0x00)                # LDA #0
    a.abs_op(0x8D, JOY1)              # STA $4016     (strobe off)
    a.emit(0x85, PRESSED)             # STA pressed   (= 0)
    a.emit(0xA2, 0x08)                # LDX #8
    a.label("read_pad")
    a.abs_op(0xAD, JOY1)              # LDA $4016
    a.emit(0x29, 0x01)                # AND #1
    a.emit(0x05, PRESSED)             # ORA pressed
    a.emit(0x85, PRESSED)             # STA pressed
    a.emit(0xCA)                      # DEX
    a.branch(0xD0, "read_pad")        # BNE read_pad
    a.emit(0xA5, PRESSED)             # LDA pressed   (0 or 1)
    a.emit(0x0A, 0x0A, 0x0A, 0x0A, 0x0A)  # ASL x5   (0 or $20)
    a.emit(0x05, HUE)                 # ORA hue
    a.emit(0x09, 0x10)                # ORA #$10      ($11..$1C, pressed $31..$3C)
    a.emit(0xA2, 0x3F)                # LDX #$3F
    a.abs_op(0x8E, PPUADDR)           # STX PPUADDR
    a.emit(0xA2, 0x00)                # LDX #0
    a.abs_op(0x8E, PPUADDR)           # STX PPUADDR   (-> $3F00, backdrop)
    a.abs_op(0x8D, PPUDATA)           # STA PPUDATA
    a.abs_op(0x8E, PPUADDR)           # STX PPUADDR
    a.abs_op(0x8E, PPUADDR)           # STX PPUADDR   (-> $0000)
    a.emit(0x68)                      # PLA
    a.label("irq")
    a.emit(0x40)                      # RTI
    return a.resolve(), a.labels


def build_rom():
    code, labels = build_program()
    prg = bytearray(PRG_SIZE)
    prg[:len(code)] = code
    vectors = [labels["nmi"], labels["reset"], labels["irq"]]
    prg[0x3FFA:0x4000] = b"".join(bytes([lo(v), hi(v)]) for v in vectors)
    # 1 x 16 KiB PRG, 1 x 8 KiB CHR, flags 6: vertical mirroring + battery, mapper 0.
    header = b"NES\x1a" + bytes([1, 1, 0x03, 0x00]) + bytes(8)
    return header + bytes(prg) + bytes(CHR_SIZE)


def main():
    default_out = pathlib.Path(__file__).resolve().parent.parent / "nostyra-test.nes"
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_out
    rom = build_rom()
    out.write_bytes(rom)
    print(f"wrote {out} ({len(rom)} bytes, sha1 {hashlib.sha1(rom).hexdigest()})")


if __name__ == "__main__":
    main()
