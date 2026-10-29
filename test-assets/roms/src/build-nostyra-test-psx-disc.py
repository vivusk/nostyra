#!/usr/bin/env python3
"""Builds the Nostyra PlayStation test discs: self-written, GPLv3, no toolchain.

Same approach as the other test ROM builders: Python 3 only, byte-for-byte
reproducible output, hand-assembled MIPS R3000 instructions with mnemonics in the
comments. Outputs (next to this script's parent folder):
  nostyra-test-psx.bin/.cue        disc 1, serial NSTY-00001
  nostyra-test-psx-disc2.bin/.cue  disc 2, serial NSTY-00002 (other colour base)
  nostyra-test-psx.m3u             both discs

What the program does (each feature backs a runner test on PCSX ReARMed, HLE BIOS):
- Video: 320x240 NTSC; every VBlank a full-screen FillRect takes 8 x the frame
  counter (mod 256) as its red channel (consecutive frames differ even in RGB565),
  disc 2 adds a blue base.
- RAM: the frame counter word at 0x800F0000 (RetroAchievements address 0x0F0000).
- Input: pad 1 read every frame straight from the SIO port (no BIOS pad driver):
  Cross held sets green to 0xFF, L2 held sets blue to 0xFF.
- Audio: SPU voice 0 loops one 16-byte ADPCM block (a loud square wave).
VBlank is polled in I_STAT with every interrupt masked (I_MASK = 0).

Disc: ISO9660 (PVD at sector 16, root directory at 18, SYSTEM.CNF and the PS-X EXE
NSTY_000.0N), MODE2/2352 Form 1 sectors, EDC/ECC left zero (emulators do not check
them). Sector 4 carries our own text naming "Sony Computer Entertainment America"
(Nostyra's PlayStation probe and region check look for it); no Sony data is used.

Usage: python build-nostyra-test-psx-disc.py [output folder]
"""
import hashlib
import pathlib
import struct
import sys

LOAD, STACK = 0x80010000, 0x801FFFF0
FRAME_COUNTER = 0x800F0000  # RAM word (RetroAchievements address 0x0F0000)
LICENCE = b"Nostyra test disc - not licensed by Sony Computer Entertainment America"
SECTORS = 64  # padded image size; PCSX opens images of 16+ sectors
# Registers.
ZERO, AT, V0, T0, T1, T2, T3, T4, T5, S0, S1, S7, RA = 0, 1, 2, 8, 9, 10, 11, 12, 13, 16, 17, 23, 31
# I/O offsets from 0x1F800000 (held in $s7).
I_STAT, I_MASK = 0x1070, 0x1074
GP0, GP1 = 0x1810, 0x1814
JOY_DATA, JOY_STAT, JOY_MODE, JOY_CTRL, JOY_BAUD = 0x1040, 0x1044, 0x1048, 0x104A, 0x104E
SPU_VOICE0, SPU_MAIN_VOL, SPU_KON = 0x1C00, 0x1D80, 0x1D88
SPU_ADDR, SPU_FIFO, SPU_CNT, SPU_STAT = 0x1DA6, 0x1DA8, 0x1DAA, 0x1DAE
SPU_SAMPLE = 0x1000  # SPU RAM address of the tone block (past the capture buffers)
PAD_CROSS, PAD_L2 = 0x4000, 0x0100  # buttons after inverting (low byte | high << 8)


class Mips:
    """Tiny MIPS I emitter: labels with branch/jump fixups; delay slots are explicit."""

    def __init__(self, base):
        self.base, self.words, self.labels, self.fixups = base, [], {}, []

    def emit(self, word):
        self.words.append(word & 0xFFFFFFFF)

    def label(self, name):
        self.labels[name] = self.base + 4 * len(self.words)

    def i(self, op, rs, rt, imm):
        self.emit((op << 26) | (rs << 21) | (rt << 16) | (imm & 0xFFFF))

    def r(self, funct, rs=0, rt=0, rd=0, sh=0):
        self.emit((rs << 21) | (rt << 16) | (rd << 11) | (sh << 6) | funct)

    def nop(self):
        self.emit(0)

    def li(self, rt, value):  # lui rt, hi ; ori rt, rt, lo
        self.i(0x0F, 0, rt, value >> 16)
        self.i(0x0D, rt, rt, value)

    def branch(self, op, rs, rt, target):  # beq / bne + delay-slot nop
        self.fixups.append((len(self.words), "b", target))
        self.i(op, rs, rt, 0)
        self.nop()

    def jump(self, op, target):  # j (2) / jal (3) + delay-slot nop
        self.fixups.append((len(self.words), "j", target))
        self.emit(op << 26)
        self.nop()

    def code(self):
        for index, kind, target in self.fixups:
            dest = self.labels[target]
            if kind == "b":
                offset = (dest - (self.base + 4 * index + 4)) // 4
                self.words[index] |= offset & 0xFFFF
            else:
                self.words[index] |= (dest & 0x0FFFFFFF) >> 2
        return b"".join(struct.pack("<I", w) for w in self.words)


def sw(m, rt, off, base=S7):
    m.i(0x2B, base, rt, off)  # sw rt, off(base)


def sh(m, rt, off, base=S7):
    m.i(0x29, base, rt, off)  # sh rt, off(base)


def put(m, value, off, half=False):
    """Stores a constant into an I/O register (via $t0)."""
    m.li(T0, value)
    (sh if half else sw)(m, T0, off)


def wait_bits(m, label, off, mask, want_set, timeout_label=None):
    """Polls a 16-bit register until (reg & mask) != 0 (want_set) or == 0."""
    m.label(label)
    if timeout_label:
        m.i(0x09, T5, T5, -1 & 0xFFFF)  # addiu t5, t5, -1
        m.branch(0x04, T5, ZERO, timeout_label)  # beq t5, zero, timeout
    m.i(0x25, S7, T1, off)  # lhu t1, off(s7)
    m.nop()  # load delay
    m.i(0x0C, T1, T1, mask)  # andi t1, t1, mask
    m.branch(0x04 if want_set else 0x05, T1, ZERO, label)  # beq/bne t1, zero, loop


def program(disc):
    m = Mips(LOAD)
    m.i(0x0F, 0, S7, 0x1F80)  # lui s7, 0x1F80 (I/O base)
    sw(m, ZERO, I_MASK)  # every interrupt masked: VBlank is polled
    for gp1 in (0x00000000, 0x08000001, 0x05000000, 0x06C60260, 0x07040010):
        put(m, gp1, GP1)  # reset, 320x240 NTSC 15-bit, display start, h/v ranges
    for gp0 in (0xE1000400, 0xE3000000, 0xE403BD3F, 0xE5000000):
        put(m, gp0, GP0)  # draw to display area, drawing area 0,0-319,239, offset 0
    put(m, 0x03000000, GP1)  # display on
    # SPU: upload the tone block by manual FIFO write, then key on voice 0.
    put(m, 0xC000, SPU_CNT, half=True)  # SPU on, unmuted, transfers stopped
    put(m, SPU_SAMPLE // 8, SPU_ADDR, half=True)
    block = bytes([0x00, 0x07]) + bytes([0x77] * 7 + [0x88] * 7)  # loop start+repeat+end
    for k in range(0, 16, 2):
        put(m, block[k] | block[k + 1] << 8, SPU_FIFO, half=True)
    put(m, 0xC010, SPU_CNT, half=True)  # manual write: FIFO → SPU RAM
    m.li(T5, 0x10000)
    wait_bits(m, "spu_busy", SPU_STAT, 0x0400, want_set=False, timeout_label="spu_done")
    m.label("spu_done")
    put(m, 0xC000, SPU_CNT, half=True)
    for off, value in ((0x0, 0x3FFF), (0x2, 0x3FFF), (0x4, 0x1000), (0x6, SPU_SAMPLE // 8),
                       (0x8, 0x000F), (0xA, 0x0000), (0xE, SPU_SAMPLE // 8)):
        put(m, value, SPU_VOICE0 + off, half=True)  # volume, pitch, start, ADSR, repeat
    put(m, 0x3FFF, SPU_MAIN_VOL, half=True)
    put(m, 0x3FFF, SPU_MAIN_VOL + 2, half=True)
    put(m, 0x0001, SPU_KON, half=True)
    m.r(0x21, ZERO, ZERO, S0)  # addu s0, zero, zero (frame counter)
    m.label("frame")
    m.label("vblank")
    m.i(0x23, S7, T0, I_STAT)  # lw t0, I_STAT(s7)
    m.nop()
    m.i(0x0C, T0, T0, 1)  # andi t0, t0, 1 (VBlank request)
    m.branch(0x04, T0, ZERO, "vblank")
    m.li(T1, 0xFFFFFFFE)
    sw(m, T1, I_STAT)  # acknowledge VBlank
    m.i(0x09, S0, S0, 1)  # addiu s0, s0, 1
    m.i(0x0F, 0, T0, FRAME_COUNTER >> 16)  # lui t0, 0x800F
    sw(m, S0, FRAME_COUNTER & 0xFFFF, T0)  # sw s0, 0(t0) (frame counter in RAM)
    m.jump(3, "read_pad")  # jal read_pad → v0 = held buttons
    m.i(0x0C, S0, T2, 0x1F)  # andi t2, s0, 0x1F
    m.r(0x00, 0, T2, T2, 3)  # sll t2, t2, 3 (red = 8 × counter: differs in RGB565 too)
    m.li(T3, 0x02000000 | (0x400000 if disc == 2 else 0))
    m.r(0x25, T2, T3, T2)  # or t2, t2, t3
    m.i(0x0C, V0, T4, PAD_CROSS)  # andi t4, v0, CROSS
    m.branch(0x04, T4, ZERO, "no_cross")
    m.i(0x0D, T2, T2, 0xFF00)  # ori t2, t2, 0xFF00 (green)
    m.label("no_cross")
    m.i(0x0C, V0, T4, PAD_L2)  # andi t4, v0, L2
    m.branch(0x04, T4, ZERO, "no_l2")
    m.li(T3, 0x00FF0000)
    m.r(0x25, T2, T3, T2)  # or t2, t2, t3 (blue)
    m.label("no_l2")
    sw(m, T2, GP0)  # GP0(02h) fill rectangle, colour
    sw(m, ZERO, GP0)  # top-left 0,0
    put(m, 0x00F00140, GP0)  # 320x240
    m.jump(2, "frame")
    # read_pad: one SIO exchange 01 42 00 00 00 with pad 1; returns buttons in v0
    # (1 = held), 0 when no pad answers (timeouts).
    m.label("read_pad")
    put(m, 0x000D, JOY_MODE, half=True)
    put(m, 0x0088, JOY_BAUD, half=True)
    put(m, 0x0003, JOY_CTRL, half=True)  # TX enable, select pad 1
    m.r(0x21, ZERO, ZERO, S1)  # addu s1, zero, zero (response bytes 3/4)
    for n, tx in enumerate((0x01, 0x42, 0x00, 0x00, 0x00)):
        m.li(T5, 0x4000)
        wait_bits(m, f"tx{n}", JOY_STAT, 0x0001, want_set=True, timeout_label="pad_none")
        m.i(0x09, ZERO, T0, tx)  # addiu t0, zero, tx
        m.i(0x28, S7, T0, JOY_DATA)  # sb t0, JOY_DATA(s7)
        wait_bits(m, f"rx{n}", JOY_STAT, 0x0002, want_set=True, timeout_label="pad_none")
        m.i(0x24, S7, T0, JOY_DATA)  # lbu t0, JOY_DATA(s7)
        m.nop()
        if n >= 3:
            m.r(0x00, 0, T0, T0, 8 * (n - 3))  # sll t0, t0, 0/8
            m.r(0x25, S1, T0, S1)  # or s1, s1, t0
    sh(m, ZERO, JOY_CTRL)
    m.li(T0, 0xFFFF)
    m.r(0x26, S1, T0, V0)  # xor v0, s1, t0 (active-low → 1 = held)
    m.r(0x08, RA)  # jr ra
    m.nop()
    m.label("pad_none")
    sh(m, ZERO, JOY_CTRL)
    m.r(0x21, ZERO, ZERO, V0)  # addu v0, zero, zero
    m.r(0x08, RA)  # jr ra
    m.nop()
    return m.code()


def ps_exe(code):
    text = code + b"\0" * (-len(code) % 2048)
    head = bytearray(2048)
    head[:8] = b"PS-X EXE"
    struct.pack_into("<IIII", head, 0x10, LOAD, 0, LOAD, len(text))
    struct.pack_into("<II", head, 0x30, STACK, 0)
    marker = b"Nostyra test disc, GPL-3.0+"
    head[0x4C:0x4C + len(marker)] = marker  # same length: the header stays 2048 bytes
    return bytes(head) + text


def both(fmt, value):
    """ISO9660 both-endian field: little-endian then big-endian."""
    return struct.pack("<" + fmt, value) + struct.pack(">" + fmt, value)


def dir_record(lba, size, name, is_dir=False):
    rec = bytearray(33 + len(name) + (len(name) + 1) % 2)
    rec[0] = len(rec)
    rec[2:18] = both("I", lba) + both("I", size)
    rec[25] = 2 if is_dir else 0
    rec[28:32] = both("H", 1)  # volume sequence number
    rec[32] = len(name)
    rec[33:33 + len(name)] = name
    return bytes(rec)


def iso_sectors(disc):
    cnf = (f"BOOT = cdrom:\\NSTY_000.0{disc};1\r\nTCB = 4\r\nEVENT = 10\r\n"
           f"STACK = {STACK:08X}\r\n").encode()
    exe = ps_exe(program(disc))
    files = [(b"SYSTEM.CNF;1", 19, cnf), (f"NSTY_000.0{disc};1".encode(), 20, exe)]
    sectors = [bytearray(2048) for _ in range(SECTORS)]
    sectors[4][32:32 + len(LICENCE)] = LICENCE
    root = dir_record(18, 2048, b"\0", True) + dir_record(18, 2048, b"\1", True)
    for name, lba, data in files:
        root += dir_record(lba, len(data), name)
        for k in range(0, len(data), 2048):
            chunk = data[k:k + 2048]
            sectors[lba + k // 2048][:len(chunk)] = chunk
    sectors[18][:len(root)] = root
    pvd = sectors[16]
    pvd[0:8] = b"\x01CD001\x01\x00"
    pvd[8:40] = b"PLAYSTATION".ljust(32)
    pvd[40:72] = f"NOSTYRA_TEST_{disc}".encode().ljust(32)
    pvd[80:88] = both("I", SECTORS)  # volume space size
    pvd[120:132] = both("H", 1) + both("H", 1) + both("H", 2048)  # set size, seq, block
    pvd[156:190] = dir_record(18, 2048, b"\0", True)
    sectors[17][0:7] = b"\xffCD001\x01"
    last = {19 + (len(cnf) - 1) // 2048, 20 + (len(exe) - 1) // 2048}
    return sectors, last


def raw_image(sectors, last_sectors):
    out = bytearray()
    for lba, data in enumerate(sectors):
        frame = lba + 150
        bcd = [(v // 10) << 4 | v % 10 for v in (frame // 4500, frame // 75 % 60, frame % 75)]
        sub = 0x89 if lba in last_sectors else 0x08
        out += (b"\x00" + b"\xff" * 10 + b"\x00" + bytes(bcd) + b"\x02"
                + bytes([0, 0, sub, 0] * 2) + data + b"\0" * 280)
    return bytes(out)


def main():
    out_dir = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else
                           pathlib.Path(__file__).resolve().parent.parent)
    for disc, stem in ((1, "nostyra-test-psx"), (2, "nostyra-test-psx-disc2")):
        image = raw_image(*iso_sectors(disc))
        (out_dir / f"{stem}.bin").write_bytes(image)
        cue = f'FILE "{stem}.bin" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n'
        (out_dir / f"{stem}.cue").write_bytes(cue.encode())
        print(f"{hashlib.sha1(image).hexdigest()}  {stem}.bin")
    (out_dir / "nostyra-test-psx.m3u").write_bytes(
        b"nostyra-test-psx.cue\nnostyra-test-psx-disc2.cue\n")


if __name__ == "__main__":
    main()
