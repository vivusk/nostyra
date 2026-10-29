#!/usr/bin/env python3
"""Builds nostyra-test.sfc: a tiny, self-written, GPLv3 SNES test ROM.

Why a Python builder instead of ca65/WLA-DX: zero external toolchain, the
output is byte-for-byte reproducible on any OS with Python 3, and the ROM is
small enough that hand-assembled opcodes (mnemonics in comments) stay readable.

What the ROM does (each feature backs a runner test):
- Video: backdrop color ramps with a frame counter (deterministic frames).
- Input: holding any joypad button tints the color (input mapping check).
- Audio: uploads an SPC700 program that plays a ~440 Hz square wave.
- SRAM: 2 KiB battery RAM; byte $70:0000 is a boot counter, +1 per reset
  (proves .srm load/save round-trips across runs).

Usage: python build-nostyra-test-rom.py [output.sfc]
"""
import pathlib
import sys

ROM_SIZE = 0x8000  # 32 KiB LoROM, one bank mapped at $00:8000
ORIGIN = 0x8000


class Asm:
    """Minimal byte emitter with labels and 8-bit relative branch fixups."""

    def __init__(self, origin):
        self.origin = origin
        self.buf = bytearray()
        self.labels = {}
        self.fixups = []  # (offset of rel byte, label)

    @property
    def pc(self):
        return self.origin + len(self.buf)

    def label(self, name):
        self.labels[name] = self.pc

    def emit(self, *data):
        self.buf.extend(data)

    def branch(self, opcode, name):
        self.emit(opcode, 0)
        self.fixups.append((len(self.buf) - 1, name))

    def resolve(self):
        for off, name in self.fixups:
            rel = self.labels[name] - (self.origin + off + 1)
            if not -128 <= rel <= 127:
                raise ValueError(f"branch to {name} out of range")
            self.buf[off] = rel & 0xFF
        return bytes(self.buf)


def lo(v):
    return v & 0xFF


def hi(v):
    return (v >> 8) & 0xFF


# ---------------------------------------------------------------- SPC700 side
SPC_BASE = 0x0200  # upload target in APU RAM; DIR page must be page-aligned
SPC_DIR_PAGE = 0x02  # sample directory at $0200
SPC_BRR_ADDR = 0x0204
SPC_ENTRY = 0x0210


def build_spc_blob():
    """Directory + one looping BRR block (square wave) + DSP init code."""
    blob = bytearray()
    # Directory entry 0: sample start, loop start (both the single BRR block).
    blob += bytes([lo(SPC_BRR_ADDR), hi(SPC_BRR_ADDR)] * 2)
    # BRR block: header shift=12, filter=0, loop+end; 16 nibbles = one period
    # (8 x +7, 8 x -7) -> pitch $0385 gives ~440 Hz.
    blob += bytes([0xC3] + [0x77] * 4 + [0x99] * 4)
    blob += bytes(SPC_ENTRY - SPC_BASE - len(blob))  # pad to entry point
    dsp_writes = [
        (0x6C, 0x20),  # FLG: unmute, no soft reset, echo writes off
        (0x0C, 0x7F), (0x1C, 0x7F),  # MVOL L/R
        (0x2C, 0x00), (0x3C, 0x00),  # EVOL L/R
        (0x2D, 0x00), (0x3D, 0x00), (0x4D, 0x00),  # PMON, NON, EON off
        (0x7D, 0x00),  # EDL: no echo delay
        (0x5D, SPC_DIR_PAGE),  # DIR
        (0x00, 0x40), (0x01, 0x40),  # V0 VOL L/R (half, avoids clipping)
        (0x02, 0x85), (0x03, 0x03),  # V0 PITCH = $0385
        (0x04, 0x00),  # V0 SRCN = sample 0
        (0x05, 0x00),  # V0 ADSR1: ADSR off -> use GAIN
        (0x07, 0x7F),  # V0 GAIN: direct, max
        (0x5C, 0x00),  # KOF
        (0x4C, 0x01),  # KON voice 0
    ]
    for reg, val in dsp_writes:
        blob += bytes([0x8F, reg, 0xF2])  # mov $F2,#reg
        blob += bytes([0x8F, val, 0xF3])  # mov $F3,#val
    blob += bytes([0x2F, 0xFE])  # bra * (idle forever; DSP keeps playing)
    return bytes(blob)


# ---------------------------------------------------------------- 65816 side
def build_program(spc_blob):
    a = Asm(ORIGIN)
    a.label("reset")
    a.emit(0x78)  # sei
    a.emit(0x18, 0xFB)  # clc ; xce  -> native mode
    a.emit(0xE2, 0x30)  # sep #$30   -> 8-bit A/X/Y
    a.emit(0xA9, 0x80, 0x8D, 0x00, 0x21)  # lda #$80 ; sta $2100 (force blank)
    a.emit(0x9C, 0x00, 0x42)  # stz $4200 (no NMI/auto-joypad yet)
    for reg in (0x2105, 0x212C, 0x212D, 0x2130, 0x2131):  # mode 0, no layers, no color math
        a.emit(0x9C, lo(reg), hi(reg))  # stz reg
    # SRAM boot counter: $70:0000 += 1
    a.emit(0xAF, 0x00, 0x00, 0x70)  # lda $700000
    a.emit(0x1A)  # inc a
    a.emit(0x8F, 0x00, 0x00, 0x70)  # sta $700000

    # --- Upload SPC blob through the IPL ROM handshake.
    a.label("wait_aa")
    a.emit(0xAD, 0x40, 0x21, 0xC9, 0xAA)  # lda $2140 ; cmp #$AA
    a.branch(0xD0, "wait_aa")
    a.label("wait_bb")
    a.emit(0xAD, 0x41, 0x21, 0xC9, 0xBB)  # lda $2141 ; cmp #$BB
    a.branch(0xD0, "wait_bb")
    a.emit(0xA9, lo(SPC_BASE), 0x8D, 0x42, 0x21)  # dest addr lo -> $2142
    a.emit(0xA9, hi(SPC_BASE), 0x8D, 0x43, 0x21)  # dest addr hi -> $2143
    a.emit(0xA9, 0x01, 0x8D, 0x41, 0x21)  # $2141 = 1 (transfer follows)
    a.emit(0xA9, 0xCC, 0x8D, 0x40, 0x21)  # $2140 = $CC (kick)
    a.label("wait_cc")
    a.emit(0xCD, 0x40, 0x21)  # cmp $2140
    a.branch(0xD0, "wait_cc")
    a.emit(0xA2, 0x00)  # ldx #0
    a.label("upload")
    a.emit(0xBD)  # lda spc_blob,x (abs address patched below)
    spc_ref = len(a.buf)
    a.emit(0x00, 0x00)
    a.emit(0x8D, 0x41, 0x21)  # sta $2141 (data)
    a.emit(0x8A, 0x8D, 0x40, 0x21)  # txa ; sta $2140 (index)
    a.label("wait_ack")
    a.emit(0xCD, 0x40, 0x21)  # cmp $2140
    a.branch(0xD0, "wait_ack")
    a.emit(0xE8, 0xE0, len(spc_blob))  # inx ; cpx #len
    a.branch(0xD0, "upload")
    a.emit(0xA9, lo(SPC_ENTRY), 0x8D, 0x42, 0x21)  # entry lo
    a.emit(0xA9, hi(SPC_ENTRY), 0x8D, 0x43, 0x21)  # entry hi
    a.emit(0x9C, 0x41, 0x21)  # stz $2141 (0 = jump to entry)
    a.emit(0x8A, 0x1A, 0x1A)  # txa ; inc a ; inc a  (last index + 2)
    a.emit(0x8D, 0x40, 0x21)  # sta $2140
    a.label("wait_go")
    a.emit(0xCD, 0x40, 0x21)  # cmp $2140
    a.branch(0xD0, "wait_go")

    # --- Display on, auto-joypad on.
    a.emit(0x64, 0x00)  # stz $00 (frame counter)
    a.emit(0xA9, 0x01, 0x8D, 0x00, 0x42)  # lda #$01 ; sta $4200 (auto-joypad)
    a.emit(0xA9, 0x0F, 0x8D, 0x00, 0x21)  # lda #$0F ; sta $2100 (screen on)

    # --- Main loop: once per vblank, write backdrop color to CGRAM[0].
    a.label("main")
    a.emit(0xAD, 0x12, 0x42)  # lda $4212
    a.branch(0x30, "main")  # bmi main (wait for vblank to end)
    a.label("wait_vblank")
    a.emit(0xAD, 0x12, 0x42)  # lda $4212
    a.branch(0x10, "wait_vblank")  # bpl (wait for vblank start)
    a.label("wait_joy")
    a.emit(0xAD, 0x12, 0x42, 0x29, 0x01)  # lda $4212 ; and #1
    a.branch(0xD0, "wait_joy")  # auto-joypad read still busy
    a.emit(0xE6, 0x00)  # inc $00
    a.emit(0x9C, 0x21, 0x21)  # stz $2121 (CGRAM addr 0)
    a.emit(0xA5, 0x00, 0x8D, 0x22, 0x21)  # lda $00 ; sta $2122 (color lo)
    a.emit(0xAD, 0x18, 0x42)  # lda $4218 (A X L R)
    a.emit(0x0D, 0x19, 0x42)  # ora $4219 (B Y Sel Start dpad)
    a.emit(0x4A, 0x8D, 0x22, 0x21)  # lsr a ; sta $2122 (color hi; keeps B/A in bit 6)
    a.branch(0x80, "main")  # bra main

    a.label("rti_stub")
    a.emit(0x40)  # rti (unused interrupts)
    a.label("spc_blob")
    code = bytearray(a.resolve())
    code[spc_ref:spc_ref + 2] = bytes([lo(a.labels["spc_blob"]), hi(a.labels["spc_blob"])])
    return bytes(code) + spc_blob, a.labels


def build_rom():
    spc_blob = build_spc_blob()
    if len(spc_blob) > 0xFF:
        raise ValueError("SPC blob must fit an 8-bit upload index")
    program, labels = build_program(spc_blob)
    rom = bytearray(ROM_SIZE)
    rom[: len(program)] = program
    if len(program) > 0x7FC0:
        raise ValueError("program overlaps header")

    # Internal header at $00:FFC0 (file offset $7FC0).
    rom[0x7FC0:0x7FD5] = b"NOSTYRA TEST ROM".ljust(21)
    rom[0x7FD5] = 0x20  # LoROM, SlowROM
    rom[0x7FD6] = 0x02  # ROM + RAM + battery
    rom[0x7FD7] = 0x05  # ROM size: 1 << 5 KiB = 32 KiB
    rom[0x7FD8] = 0x01  # SRAM size: 1 << 1 KiB = 2 KiB
    rom[0x7FD9] = 0x01  # region: North America (NTSC)
    rom[0x7FDA] = 0x00  # developer id
    rom[0x7FDB] = 0x00  # version
    stub, reset = labels["rti_stub"], labels["reset"]
    for off in range(0x7FE4, 0x8000, 2):  # all vectors -> rti stub
        rom[off:off + 2] = bytes([lo(stub), hi(stub)])
    rom[0x7FFC:0x7FFE] = bytes([lo(reset), hi(reset)])  # emulation RESET

    # Checksum: complement/checksum bytes always add 0x1FE, so seed then sum.
    rom[0x7FDC:0x7FE0] = bytes([0xFF, 0xFF, 0x00, 0x00])
    checksum = sum(rom) & 0xFFFF
    rom[0x7FDC:0x7FE0] = bytes([lo(checksum ^ 0xFFFF), hi(checksum ^ 0xFFFF), lo(checksum), hi(checksum)])
    return bytes(rom)


def main():
    default_out = pathlib.Path(__file__).resolve().parent.parent / "nostyra-test.sfc"
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_out
    out.write_bytes(build_rom())
    print(f"wrote {out} ({out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
