#!/usr/bin/env python3
"""Builds the Nostyra Nintendo 64 test ROM: self-written, GPLv3, no toolchain.

Same approach as the other test ROM builders: Python 3 only, byte-for-byte
reproducible output, hand-assembled MIPS instructions (R4300i, 32-bit subset) with
mnemonics in the comments. Output: nostyra-test.z64 (big-endian "z64" byte order)
next to this script's parent folder.

Boot: Mupen64Plus HLE-boots the PIF and runs the cart's 0x40-0x1000 "IPL3" from SP
DMEM without checking its checksum (unknown IPL3 -> CIC 6102 seed). Ours is a
15-instruction loader: PI-DMA the payload at ROM 0x1000 to RDRAM 0x400 and jump to
0x80000400. libdragon ROMs were not used: their IPL3 (r5+) hangs Mupen's dynarec
(mupen64plus-core issue #1185); this ROM never runs on real hardware.

What the program does (each feature backs a runner test on Mupen64Plus-Next):
- Video: VI 320x240 RGBA5551, two framebuffers swapped every VI (GLideN64 presents on
  an origin change). Each frame an RDP command list sent straight to DPC_START/END
  fills the top half red and the bottom half blue whose green channel is the frame
  counter (mod 32), so consecutive frames differ and an upside-down image shows.
- RAM: the frame counter word at RDRAM 0xF0000 (RetroAchievements address 0x0F0000).
- Input: controller 1 read through the PIF every frame: A held -> top half green;
  stick X > +40 -> bottom half white.
- Audio: AI DMA of a square wave (32 kHz, ~500 Hz) every frame the FIFO has room.
- Save: EEPROM 4K (Advanced Homebrew Header "ED", save type 1). At boot block 0 is
  read, byte 0 (boot counter) + 1 and byte 1 = 0xA5 are written back (a blank
  EEPROM, byte 1 != 0xA5, counts from 0, so the first boot writes 1).

Usage: python build-nostyra-test-n64-rom.py [output folder]
"""
import hashlib
import pathlib
import struct
import sys

ENTRY = 0x80000400  # payload is loaded to RDRAM 0x400 and runs from KSEG0
PAYLOAD_ROM = 0x1000
ROM_SIZE = 16 * 1024
FB = (0x00100000, 0x00140000)  # two 320x240x2 framebuffers (physical)
WORK = 0x000E0000  # 64-byte PIF exchange buffer (physical)
FRAME_COUNTER = 0xA00F0000  # KSEG1 word: physical 0xF0000 = RetroAchievements 0x0F0000
WIDTH, HEIGHT = 320, 240
# RGBA5551 colours.
RED, GREEN, BLUE, WHITE = 0xF801, 0x07C1, 0x003F, 0xFFFF
STICK_THRESHOLD = 40
AUDIO_FRAMES = 536  # stereo frames per AI DMA (32000 / 60, rounded to 8 bytes)
# Registers.
ZERO, V1, A0, A1 = 0, 3, 4, 5
T0, T1, T2, T3, T4, T5, T6, T7 = 8, 9, 10, 11, 12, 13, 14, 15
S0, S1, S2, S3, S4, S5, S6, S7, T8, RA = 16, 17, 18, 19, 20, 21, 22, 23, 24, 31
# RCP register blocks (KSEG1) and offsets.
DPC, MI, VI, AI, PI, SI = 0xA4100000, 0xA4300000, 0xA4400000, 0xA4500000, 0xA4600000, 0xA4800000
PIF_RAM = 0x1FC007C0


class Mips:
    """Tiny big-endian MIPS emitter: labels with branch/jump fixups; explicit delay slots."""

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
        self.i(0x0F, 0, rt, (value >> 16) & 0xFFFF)
        self.i(0x0D, rt, rt, value)

    def branch(self, op, rs, rt, target):  # beq (4) / bne (5) / regimm (1) + delay-slot nop
        self.fixups.append((len(self.words), "b", target))
        self.i(op, rs, rt, 0)
        self.nop()

    def jump(self, op, target):  # j (2) / jal (3) + delay-slot nop
        self.fixups.append((len(self.words), "j", target))
        self.emit(op << 26)
        self.nop()

    def jr(self, rs):  # jr rs + delay-slot nop
        self.r(0x08, rs=rs)
        self.nop()

    def code(self):
        for index, kind, target in self.fixups:
            dest = self.labels[target]
            if kind == "b":
                offset = (dest - (self.base + 4 * index + 4)) // 4
                self.words[index] |= offset & 0xFFFF
            else:
                self.words[index] |= (dest & 0x0FFFFFFF) >> 2
        return b"".join(struct.pack(">I", w) for w in self.words)


def lw(m, rt, off, base):
    m.i(0x23, base, rt, off)


def sw(m, rt, off, base):
    m.i(0x2B, base, rt, off)


def put(m, base_reg, off, value):
    """Stores a constant into a 32-bit register (via $t0)."""
    m.li(T0, value)
    sw(m, T0, off, base_reg)


def wait_clear(m, label, base_reg, off, mask):
    """Polls a register until (reg & mask) == 0."""
    m.label(label)
    lw(m, T1, off, base_reg)
    m.i(0x0C, T1, T1, mask)  # andi t1, t1, mask
    m.branch(0x05, T1, ZERO, label)  # bne t1, zero, loop


def loader(payload_len):
    """The 0x40-0x1000 boot stub, run from SP DMEM (0xA4000040)."""
    m = Mips(0xA4000040)
    m.li(T2, PI)
    put(m, T2, 0x00, ENTRY & 0x1FFFFFFF)  # PI_DRAM_ADDR = 0x400
    put(m, T2, 0x04, 0x10000000 + PAYLOAD_ROM)  # PI_CART_ADDR
    put(m, T2, 0x0C, payload_len - 1)  # PI_WR_LEN: cart -> RDRAM
    wait_clear(m, "pi", T2, 0x10, 0x3)  # PI_STATUS DMA/IO busy
    put(m, T2, 0x10, 0x2)  # clear the PI interrupt
    m.li(T1, ENTRY)
    m.jr(T1)
    return m.code()


def rdp_list(fb):
    """Fill-mode RDP commands; words 9 / 15 (fill colours) are patched every frame."""
    fill = lambda c: (0x37000000, (c << 16) | c)  # Set Fill Color
    rect = lambda x0, y0, x1, y1: (0x36000000 | (x1 << 2) << 12 | (y1 << 2), (x0 << 2) << 12 | (y0 << 2))
    cmds = [
        (0x3F100000 | (WIDTH - 1), fb),  # Set Color Image: RGBA, 16-bit, width 320
        (0x2D000000, (WIDTH << 2) << 12 | (HEIGHT << 2)),  # Set Scissor (0,0)-(320,240)
        (0x27000000, 0),  # Sync Pipe
        (0x2F300000, 0),  # Set Other Modes: cycle type FILL
        fill(RED),
        rect(0, 0, WIDTH - 1, HEIGHT // 2 - 1),  # top half (fill mode: inclusive)
        (0x27000000, 0),  # Sync Pipe
        fill(BLUE),
        rect(0, HEIGHT // 2, WIDTH - 1, HEIGHT - 1),  # bottom half
        (0x29000000, 0),  # Sync Full
    ]
    return b"".join(struct.pack(">II", a, b) for a, b in cmds)


def pif_block(cmd_bytes):
    """A 64-byte PIF RAM image: commands, 0xFE end marker, control byte 1 (run joybus)."""
    block = bytearray(64)
    block[: len(cmd_bytes)] = cmd_bytes
    block[len(cmd_bytes)] = 0xFE
    block[63] = 0x01
    return bytes(block)


# Controller 1 read: dummy 0xFF, tx 1, rx 4, cmd 0x01, 4 response bytes (buttons, x, y).
PIF_PAD = pif_block(bytes([0xFF, 0x01, 0x04, 0x01, 0xFF, 0xFF, 0xFF, 0xFF]))
# EEPROM (channel 4: skip channels 0-3): read block 0 -> 8 bytes at offset 8.
PIF_EEP_READ = pif_block(bytes([0, 0, 0, 0, 0x02, 0x08, 0x04, 0x00]) + bytes(8))
# EEPROM write block 0: 8 data bytes at offset 8 (patched), 1 status byte.
PIF_EEP_WRITE = pif_block(bytes([0, 0, 0, 0, 0x0A, 0x01, 0x05, 0x00]) + bytes(8) + bytes(1))


def square_wave():
    out = bytearray()
    for n in range(AUDIO_FRAMES):
        v = 0x2000 if (n // 32) % 2 == 0 else -0x2000
        out += struct.pack(">hh", v, v)
    return bytes(out)


def program():
    """Returns the payload (code + data) loaded at ENTRY."""
    # Data layout after the code (offsets fixed up once the code size is known).
    m = Mips(ENTRY)
    data = {}  # name -> KSEG0 address, filled by layout()

    def addr(name):
        return data[name]

    def build(m):
        m.li(S2, VI)
        m.li(S3, MI)
        m.li(S4, SI)
        m.li(S5, AI)
        m.li(S6, DPC)
        m.li(S7, 0xA0000000 | WORK)
        m.i(0x09, ZERO, S0, 0)  # addiu s0, zero, 0   frame counter
        m.i(0x09, ZERO, S1, 0)  # addiu s1, zero, 0   framebuffer index
        put(m, S3, 0x0C, 0x0555)  # MI_INTR_MASK: clear every mask bit
        # VI: NTSC 320x240 RGBA5551, no gamma/dither, AA off (replicate).
        for off, value in [
            (0x04, FB[0]), (0x08, WIDTH), (0x0C, 2), (0x14, 0x03E52239), (0x18, 0x20D),
            (0x1C, 0x0C15), (0x20, 0x0C150C15), (0x24, 0x006C02EC), (0x28, 0x002501FF),
            (0x2C, 0x000E0204), (0x30, 0x200), (0x34, 0x400), (0x00, 0x00003302),
        ]:
            put(m, S2, off, value)
        # AI: 32 kHz from the NTSC VI clock, DMA on.
        put(m, S5, 0x10, round(48681812 / 32000) - 1)  # AI_DACRATE
        put(m, S5, 0x14, 15)  # AI_BITRATE
        put(m, S5, 0x08, 1)  # AI_CONTROL
        # EEPROM boot counter: read block 0, counter + 1, marker 0xA5, write back.
        m.li(A1, addr("eep_read") | 0x20000000)  # KSEG1 view of the template
        m.jump(0x03, "pif_xfer")  # jal pif_xfer
        m.i(0x24, S7, T2, 8)  # lbu t2, 8(s7)       counter read from EEPROM
        m.i(0x24, S7, T3, 9)  # lbu t3, 9(s7)       marker
        m.i(0x09, ZERO, T4, 0xA5)  # addiu t4, zero, 0xA5
        m.branch(0x04, T3, T4, "eep_valid")  # beq t3, t4, eep_valid
        m.i(0x09, ZERO, T2, 0)  # addiu t2, zero, 0   blank EEPROM (0xFF): start at 0
        m.label("eep_valid")
        m.li(A1, addr("eep_write") | 0x20000000)
        m.jump(0x03, "copy64")  # jal copy64 (template -> work)
        m.i(0x09, T2, T2, 1)  # addiu t2, t2, 1
        m.i(0x28, S7, T2, 8)  # sb t2, 8(s7)
        m.i(0x09, ZERO, T2, 0xA5)  # addiu t2, zero, 0xA5
        m.i(0x28, S7, T2, 9)  # sb t2, 9(s7)
        m.jump(0x03, "si_xfer")  # jal si_xfer (work as is)

        m.label("frame")
        # Wait for the VI interrupt line (MI_INTR bit 3), then acknowledge it.
        m.label("vi_wait")
        lw(m, T1, 0x08, S3)  # lw t1, MI_INTR
        m.i(0x0C, T1, T1, 0x08)  # andi t1, t1, 8
        m.branch(0x04, T1, ZERO, "vi_wait")  # beq t1, zero, vi_wait
        sw(m, ZERO, 0x10, S2)  # VI_CURRENT = 0: clear the VI interrupt
        # Controller 1.
        m.li(A1, addr("pad") | 0x20000000)
        m.jump(0x03, "pif_xfer")
        m.i(0x24, S7, T1, 4)  # lbu t1, 4(s7)   buttons high byte (A = 0x80)
        m.i(0x20, S7, T2, 6)  # lb  t2, 6(s7)   stick X (signed)
        # Top colour: A held -> green, else red.
        m.li(T3, (RED << 16) | RED)
        m.i(0x0C, T1, T1, 0x80)  # andi t1, t1, 0x80
        m.branch(0x04, T1, ZERO, "top_done")
        m.li(T3, (GREEN << 16) | GREEN)
        m.label("top_done")
        # Bottom colour: stick X > threshold -> white, else blue | (frame & 31) << 6.
        m.i(0x0C, S0, T4, 31)  # andi t4, s0, 31
        m.r(0x00, rt=T4, rd=T4, sh=6)  # sll t4, t4, 6
        m.i(0x0D, T4, T4, BLUE)  # ori t4, t4, BLUE
        m.r(0x00, rt=T4, rd=T5, sh=16)  # sll t5, t4, 16
        m.r(0x25, rs=T4, rt=T5, rd=T4)  # or t4, t4, t5
        m.i(0x0A, T2, T5, STICK_THRESHOLD + 1)  # slti t5, t2, 41
        m.branch(0x05, T5, ZERO, "bottom_done")  # bne t5, zero, bottom_done
        m.li(T4, (WHITE << 16) | WHITE)
        m.label("bottom_done")
        # Patch the RDP list: colour image, two fill colours.
        m.li(T6, addr("rdp") | 0x20000000)
        m.li(T7, FB[0])
        m.branch(0x04, S1, ZERO, "fb_done")  # beq s1, zero, fb_done
        m.li(T7, FB[1])
        m.label("fb_done")
        sw(m, T7, 4, T6)
        sw(m, T3, 9 * 4, T6)
        sw(m, T4, 15 * 4, T6)
        # Run it: RDRAM source, not frozen; START / END are physical addresses.
        put(m, S6, 0x0C, 0x5)  # DPC_STATUS: clear XBUS, clear freeze
        m.li(T0, addr("rdp") & 0x1FFFFFFF)
        sw(m, T0, 0x00, S6)  # DPC_START
        m.i(0x09, T0, T0, len(rdp_list(0)))  # addiu t0, t0, list length
        sw(m, T0, 0x04, S6)  # DPC_END
        put(m, S3, 0x00, 0x0800)  # MI_MODE: clear the DP interrupt
        sw(m, T7, 0x04, S2)  # VI_ORIGIN = the frame just drawn
        m.i(0x0E, S1, S1, 1)  # xori s1, s1, 1
        # Audio: queue one buffer when the AI FIFO is not full.
        lw(m, T1, 0x0C, S5)  # lw t1, AI_STATUS
        m.branch(0x01, T1, 0, "audio_done")  # bltz t1 (full), audio_done
        put(m, S5, 0x00, addr("audio") & 0x1FFFFFFF)  # AI_DRAM_ADDR
        put(m, S5, 0x04, AUDIO_FRAMES * 4)  # AI_LEN starts the DMA
        sw(m, ZERO, 0x0C, S5)  # AI_STATUS write: clear the AI interrupt
        m.label("audio_done")
        m.i(0x09, S0, S0, 1)  # addiu s0, s0, 1
        m.li(T0, FRAME_COUNTER)
        sw(m, S0, 0, T0)  # sw s0, 0(t0) (frame counter word in RDRAM)
        m.jump(0x02, "frame")

        # copy64: 64 bytes from $a1 (KSEG1) to the work buffer ($s7). Leaf.
        m.label("copy64")
        m.i(0x09, ZERO, T6, 16)  # addiu t6, zero, 16
        m.i(0x09, S7, A0, 0)  # addiu a0, s7, 0
        m.label("copy_loop")
        lw(m, T7, 0, A1)
        sw(m, T7, 0, A0)
        m.i(0x09, A1, A1, 4)
        m.i(0x09, A0, A0, 4)
        m.i(0x09, T6, T6, -1)
        m.branch(0x05, T6, ZERO, "copy_loop")
        m.jr(RA)

        # pif_xfer: copy64 then si_xfer (keeps $ra in $t8).
        m.label("pif_xfer")
        m.i(0x09, RA, T8, 0)  # addiu t8, ra, 0
        m.jump(0x03, "copy64")
        m.jump(0x03, "si_xfer")
        m.jr(T8)

        # si_xfer: work buffer -> PIF RAM (runs the commands) -> work buffer. Leaf.
        m.label("si_xfer")
        wait_clear(m, "si_w0", S4, 0x18, 0x3)  # SI_STATUS DMA/IO busy
        put(m, S4, 0x00, WORK)  # SI_DRAM_ADDR
        put(m, S4, 0x10, PIF_RAM)  # SI_PIF_ADDR_WR64B: RDRAM -> PIF
        wait_clear(m, "si_w1", S4, 0x18, 0x3)
        put(m, S4, 0x00, WORK)
        put(m, S4, 0x04, PIF_RAM)  # SI_PIF_ADDR_RD64B: PIF -> RDRAM
        wait_clear(m, "si_w2", S4, 0x18, 0x3)
        sw(m, ZERO, 0x18, S4)  # SI_STATUS write: clear the SI interrupt
        m.jr(RA)

    # Two passes: the first sizes the code, the second uses the real data addresses.
    for name in ("eep_read", "eep_write", "pad", "rdp", "audio"):
        data[name] = ENTRY
    build(m)
    code_len = (len(m.words) * 4 + 63) & ~63
    blobs = [("eep_read", PIF_EEP_READ), ("eep_write", PIF_EEP_WRITE), ("pad", PIF_PAD),
             ("rdp", None), ("audio", square_wave())]
    offset = code_len
    for name, blob in blobs:
        data[name] = ENTRY + offset
        offset += len(blob if blob is not None else rdp_list(0))
        offset = (offset + 63) & ~63
    m = Mips(ENTRY)
    build(m)
    payload = bytearray(m.code().ljust(code_len, b"\0"))
    for name, blob in blobs:
        start = data[name] - ENTRY
        payload = payload.ljust(start, b"\0")
        payload += blob if blob is not None else rdp_list(FB[0])
    return bytes(payload.ljust((len(payload) + 63) & ~63, b"\0"))


def header():
    h = bytearray(0x40)
    struct.pack_into(">IIII", h, 0, 0x80371240, 0x0000000F, ENTRY, 0x0000144C)
    struct.pack_into(">II", h, 0x10, 0x4E4F5354, 0x59524131)  # "NOSTYRA1": no real CRC
    h[0x20:0x34] = b"NOSTYRA TEST".ljust(20, b" ")
    h[0x3B:0x3F] = b"NEDE"  # category N, Advanced Homebrew Header "ED", region E (NTSC)
    h[0x3F] = 0x10  # homebrew header: save type 1 = EEPROM 4K
    return bytes(h)


def build_rom():
    payload = program()
    stub = loader(len(payload))
    rom = bytearray(ROM_SIZE)
    rom[0:0x40] = header()
    assert 0x40 + len(stub) <= PAYLOAD_ROM
    rom[0x40:0x40 + len(stub)] = stub
    assert PAYLOAD_ROM + len(payload) <= ROM_SIZE, len(payload)
    rom[PAYLOAD_ROM:PAYLOAD_ROM + len(payload)] = payload
    return bytes(rom)


def main():
    out_dir = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parent.parent
    rom = build_rom()
    path = out_dir / "nostyra-test.z64"
    path.write_bytes(rom)
    print(f"{path}  {len(rom)} bytes  sha1 {hashlib.sha1(rom).hexdigest()}")


if __name__ == "__main__":
    main()
