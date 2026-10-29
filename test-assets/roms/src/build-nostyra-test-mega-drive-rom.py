#!/usr/bin/env python3
"""Builds nostyra-test.gen: a tiny, self-written, GPLv3 Sega Mega Drive / Genesis test ROM.

Same approach as the other test-ROM scripts: no external toolchain, output byte-for-byte
reproducible, hand-assembled 68000 opcodes with mnemonics in comments. Only the encodings
listed in `M68k` are used; each one follows the M68000 Programmer's Reference Manual
operation-word tables (big-endian words, extension words after the opcode).

What the ROM does (each feature backs a runner test on Genesis Plus GX):
- Video: CRAM colour 0 (= backdrop and every pixel of the all-zero planes) takes a new
  value every VBlank (8-step ramp, consecutive frames always differ).
- Input: holding any of Up/Down/Left/Right/B/C (TH high read of port 1) adds blue.
- Audio: PSG channel 0 plays a constant ~440 Hz square wave at full volume.
- RAM: the frame counter (+1 per VBlank, 8 bits) at 0xFF0000 and 0xFF0001 (both bytes,
  so RetroAchievements' byte-swapped view reads it at either address).
- SRAM: header `RA` battery RAM 0x200001..0x203FFF; byte 0x200000 is a boot counter
  (+1 per reset), byte 0x200001 a constant marker (0xA5). A fresh cart (all 0xFF) is
  initialised on the first boot (counter 0 before the increment).

Format: 128 KiB, no copier header (raw / No-Intro layout, "SEGA" at 0x100), 3-button pad,
header checksum filled in. The file is committed as `.gen` so Markdown tooling never
mistakes it for a document; tests copy it to `.md` / `.bin` / `.smd` as needed.

Usage: python build-nostyra-test-mega-drive-rom.py [output.gen]
"""
import hashlib
import pathlib
import struct
import sys

ROM_SIZE = 0x20000
CODE_START = 0x200
STACK_TOP = 0x00FFFE00

VDP_DATA, VDP_CTRL, PSG = 0xC00000, 0xC00004, 0xC00011
VERSION, PAD1_DATA, PAD1_CTRL = 0xA10001, 0xA10003, 0xA10009
TMSS, SRAM_CTRL = 0xA14000, 0xA130F1
BOOT_COUNTER, MARKER = 0x200000, 0x200001
FRAME_COUNTER = 0xFF0000  # 68K RAM: RA address 0x0000 (and 0x0001)
CRAM_WRITE_0 = 0xC0000000  # VDP command: CRAM write, address 0
# PSG tone: 3579545 Hz / (32 * 440) = 254 -> latch low 4 bits, then high 6 bits.
PSG_TONE = 254
# VDP registers: mode 1 (no H-int), mode 2 (display on, Mode 5, no V-int, no DMA),
# planes A/B/window/sprites, backdrop palette 0 colour 0, H40, auto-increment 2,
# 64x32 scroll. VRAM stays all zero, so every pixel shows CRAM colour 0.
VDP_REGS = [0x8004, 0x8144, 0x8230, 0x8334, 0x8407, 0x857C, 0x8700, 0x8B00,
            0x8C81, 0x8D3F, 0x8F02, 0x9001, 0x9100, 0x9200]
COLOUR_STEP, COLOUR_MASK, PRESSED_BLUE = 0x0022, 0x00EE, 0x0E00


class Asm:
    """Big-endian word emitter with labels and 16-bit PC-relative branch fixups."""

    def __init__(self, origin):
        self.origin = origin
        self.buf = bytearray()
        self.labels = {}
        self.fixups = []  # (offset of the displacement word, label)

    @property
    def pc(self):
        return self.origin + len(self.buf)

    def label(self, name):
        self.labels[name] = self.pc

    def words(self, *ws):
        for w in ws:
            self.buf += struct.pack(">H", w & 0xFFFF)

    def long(self, v):
        self.buf += struct.pack(">I", v & 0xFFFFFFFF)

    def disp16(self, name):
        # Bcc.W / DBcc: displacement is relative to the address of the extension word.
        self.fixups.append((len(self.buf), name))
        self.words(0)

    def resolve(self):
        for off, name in self.fixups:
            rel = self.labels[name] - (self.origin + off)
            if not -0x8000 <= rel <= 0x7FFF:
                raise ValueError(f"branch to {name} out of range")
            self.buf[off:off + 2] = struct.pack(">h", rel)
        return bytes(self.buf)


class M68k(Asm):
    """The 68000 instruction subset used by this ROM (operation word per PRM)."""

    # MOVE: 00ss DDDd dd SSSsss; size b=01, w=11, l=10. abs.L = mode 7 reg 1, #imm = mode 7 reg 4.
    def move_b_imm_abs(self, imm, addr):    # move.b #imm,(addr).l
        self.words(0x13FC, imm & 0xFF)
        self.long(addr)

    def move_w_imm_abs(self, imm, addr):    # move.w #imm,(addr).l
        self.words(0x33FC, imm)
        self.long(addr)

    def move_l_imm_abs(self, imm, addr):    # move.l #imm,(addr).l
        self.words(0x23FC)
        self.long(imm)
        self.long(addr)

    def move_b_abs_d(self, addr, dn):       # move.b (addr).l,Dn
        self.words(0x1039 | dn << 9)
        self.long(addr)

    def move_w_abs_d(self, addr, dn):       # move.w (addr).l,Dn
        self.words(0x3039 | dn << 9)
        self.long(addr)

    def move_w_d_abs(self, dn, addr):       # move.w Dn,(addr).l
        self.words(0x33C0 | dn)
        self.long(addr)

    def move_w_d_d(self, src, dst):         # move.w Dsrc,Ddst
        self.words(0x3000 | dst << 9 | src)

    def move_w_imm_sr(self, imm):           # move.w #imm,SR
        self.words(0x46FC, imm)

    def moveq(self, imm, dn):               # moveq #imm,Dn
        self.words(0x7000 | dn << 9 | (imm & 0xFF))

    def addq_b_abs(self, q, addr):          # addq.b #q,(addr).l   (q 1..8)
        self.words(0x5039 | (q & 7) << 9)
        self.long(addr)

    def addi_w_d(self, imm, dn):            # addi.w #imm,Dn
        self.words(0x0640 | dn, imm)

    def andi_w_d(self, imm, dn):            # andi.w #imm,Dn
        self.words(0x0240 | dn, imm)

    def andi_b_d(self, imm, dn):            # andi.b #imm,Dn
        self.words(0x0200 | dn, imm & 0xFF)

    def ori_w_d(self, imm, dn):             # ori.w #imm,Dn
        self.words(0x0040 | dn, imm)

    def cmpi_b_d(self, imm, dn):            # cmpi.b #imm,Dn
        self.words(0x0C00 | dn, imm & 0xFF)

    def btst_imm_d(self, bit, dn):          # btst #bit,Dn
        self.words(0x0800 | dn, bit)

    def bcc(self, cond, name):              # Bcc.w label (bra=0, bne=6, beq=7)
        self.words(0x6000 | cond << 8)
        self.disp16(name)



BRA, BNE, BEQ = 0x0, 0x6, 0x7


def build_program():
    a = M68k(CODE_START)
    a.label("reset")
    a.move_w_imm_sr(0x2700)                       # move.w #$2700,SR   (supervisor, no interrupts)
    a.move_b_abs_d(VERSION, 0)                    # move.b $A10001,d0
    a.andi_b_d(0x0F, 0)                           # andi.b #$0F,d0     (hardware revision)
    a.bcc(BEQ, "no_tmss")                         # beq.w no_tmss
    a.move_l_imm_abs(0x53454741, TMSS)            # move.l #'SEGA',$A14000
    a.label("no_tmss")
    for reg in VDP_REGS:
        a.move_w_imm_abs(reg, VDP_CTRL)           # move.w #$8rvv,$C00004
    a.move_b_imm_abs(0x40, PAD1_CTRL)             # move.b #$40,$A10009 (TH = output)
    a.move_b_imm_abs(0x40, PAD1_DATA)             # move.b #$40,$A10003 (TH high: C B R L D U)
    a.move_b_imm_abs(0x80 | (PSG_TONE & 0x0F), PSG)  # PSG ch0 tone latch (low 4 bits)
    a.move_b_imm_abs(PSG_TONE >> 4, PSG)          # PSG ch0 tone data (high 6 bits)
    a.move_b_imm_abs(0x90, PSG)                   # PSG ch0 attenuation 0 (full volume)
    a.move_b_imm_abs(0x01, SRAM_CTRL)             # move.b #1,$A130F1  (SRAM on, writable)
    a.move_b_abs_d(MARKER, 0)                     # move.b $200001,d0
    a.cmpi_b_d(0xA5, 0)                           # cmpi.b #$A5,d0
    a.bcc(BEQ, "sram_ready")                      # beq.w sram_ready
    a.move_b_imm_abs(0x00, BOOT_COUNTER)          # move.b #0,$200000  (fresh cart)
    a.move_b_imm_abs(0xA5, MARKER)                # move.b #$A5,$200001
    a.label("sram_ready")
    a.addq_b_abs(1, BOOT_COUNTER)                 # addq.b #1,$200000  (boot counter)
    a.moveq(0, 1)                                 # moveq #0,d1        (colour)
    a.move_w_imm_abs(0, FRAME_COUNTER)            # move.w #0,$FF0000  (frame counter)

    a.label("wait_active")
    a.move_w_abs_d(VDP_CTRL, 0)                   # move.w $C00004,d0  (VDP status)
    a.btst_imm_d(3, 0)                            # btst #3,d0         (VBlank flag)
    a.bcc(BNE, "wait_active")                     # bne.w wait_active
    a.label("wait_vblank")
    a.move_w_abs_d(VDP_CTRL, 0)                   # move.w $C00004,d0
    a.btst_imm_d(3, 0)                            # btst #3,d0
    a.bcc(BEQ, "wait_vblank")                     # beq.w wait_vblank
    a.addq_b_abs(1, FRAME_COUNTER)                # addq.b #1,$FF0000  (frame counter,
    a.addq_b_abs(1, FRAME_COUNTER + 1)            # addq.b #1,$FF0001   both bytes)
    a.addi_w_d(COLOUR_STEP, 1)                    # addi.w #$22,d1
    a.andi_w_d(COLOUR_MASK, 1)                    # andi.w #$EE,d1     (8-step red/green ramp)
    a.move_w_d_d(1, 2)                            # move.w d1,d2
    a.move_b_abs_d(PAD1_DATA, 0)                  # move.b $A10003,d0
    a.andi_b_d(0x3F, 0)                           # andi.b #$3F,d0
    a.cmpi_b_d(0x3F, 0)                           # cmpi.b #$3F,d0     (all released = $3F)
    a.bcc(BEQ, "write_colour")                    # beq.w write_colour
    a.ori_w_d(PRESSED_BLUE, 2)                    # ori.w #$0E00,d2    (pressed: add blue)
    a.label("write_colour")
    a.move_l_imm_abs(CRAM_WRITE_0, VDP_CTRL)      # move.l #$C0000000,$C00004
    a.move_w_d_abs(2, VDP_DATA)                   # move.w d2,$C00000
    a.bcc(BRA, "wait_active")                     # bra.w wait_active

    a.label("trap")
    a.bcc(BRA, "trap")                            # bra.w trap (any exception parks here)
    return a.resolve(), a.labels


def build_header():
    h = bytearray(b" " * 0x100)

    def put(off, text):
        h[off:off + len(text)] = text

    put(0x00, b"SEGA MEGA DRIVE ")
    put(0x10, b"(C)NSTY 2026.OCT")
    put(0x20, b"NOSTYRA TEST")
    put(0x50, b"NOSTYRA TEST")
    put(0x80, b"GM 00000000-00")
    put(0x90, b"J")                               # 3-button pad only
    h[0xA0:0xA8] = struct.pack(">II", 0, ROM_SIZE - 1)
    h[0xA8:0xB0] = struct.pack(">II", 0xFF0000, 0xFFFFFF)
    h[0xB0:0xBC] = b"RA" + bytes([0xF8, 0x20]) + struct.pack(">II", 0x200001, 0x203FFF)
    put(0xF0, b"JUE")
    return h


def build_rom():
    code, labels = build_program()
    rom = bytearray(ROM_SIZE)
    vectors = [STACK_TOP, labels["reset"]] + [labels["trap"]] * 62
    rom[0:0x100] = b"".join(struct.pack(">I", v) for v in vectors)
    rom[0x100:0x200] = build_header()
    rom[CODE_START:CODE_START + len(code)] = code
    checksum = sum(struct.unpack(f">{(ROM_SIZE - 0x200) // 2}H", rom[0x200:])) & 0xFFFF
    rom[0x18E:0x190] = struct.pack(">H", checksum)
    return bytes(rom)


def main():
    default_out = pathlib.Path(__file__).resolve().parent.parent / "nostyra-test.gen"
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_out
    rom = build_rom()
    out.write_bytes(rom)
    print(f"wrote {out} ({len(rom)} bytes, sha1 {hashlib.sha1(rom).hexdigest()})")


if __name__ == "__main__":
    main()
