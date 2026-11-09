"""EE (PlayStation 2 MIPS, little-endian) test program and a plain ELF32 writer.

Part of build-nostyra-test-ps2.py (self-written, GPLv3, Python 3 std-only). Reuses
the `Allegrex` emitter of psp_test_allegrex.py: the MIPS I subset it emits runs on
the Emotion Engine unchanged.

Program (fast boot, real kernel): `SetGsCrt(interlace, NTSC, frame)` syscall, GS
privileged registers (CSR reset, SMODE2, DISPFB1, DISPLAY1, BGCOLOR, PMODE), then per
frame: wait for CSR VSINT (bit 3), acknowledge it (write bit 3), bump the colour,
write RGBAQ into the GIF packet (uncached view) and DMA the packet to the GIF
(channel 2): a full-screen sprite whose colour changes every frame. Debug block at
RAM 0x00108000: 'NSTY' then the frame counter (RetroAchievements: EE RAM is RA
address 0, so the counter is RA $108004 and its low byte $108004).

The ELF writer is PS2-shaped: ET_EXEC, EM_MIPS, one RWX PT_LOAD at BASE, no section
headers, no PSP module info (the PSP writer in psp_test_allegrex.py is PRX-shaped).
"""
import struct

from psp_test_allegrex import A0, A1, A2, T0, T1, T2, T4, T5, T6, ZERO, Allegrex

BASE = 0x00100000
V1 = 3
DEBUG_BLOCK = 0x00108000  # 'NSTY' at +0, frame counter at +4
UNCACHED = 0x20000000
GS_PRIV = 0xB2000000  # PMODE; +0x20 SMODE2, +0x70 DISPFB1, +0x80 DISPLAY1, +0xE0 BGCOLOR, +0x1000 CSR
DMAC_GIF = 0xB000A000  # D2_CHCR; +0x10 MADR, +0x20 QWC
PACKET_OFFSET = 0x400  # GIF packet inside the segment (16-byte aligned)
RGBAQ_INDEX = 8  # qword index of RGBAQ inside the packet (after the tag)
FIRST_COLOUR = 0x00FF2040  # A=0 R=0x40 G=0x20 B=0xFF; red ramps per frame
SEG_OFFSET = 0x80  # file offset of the PT_LOAD segment


def ad(reg, value):
    return struct.pack("<QQ", value & 0xFFFFFFFFFFFFFFFF, reg)


def gif_packet(colour):
    """PACKED A+D: frame 0 FBW 10 CT32, Z masked, scissor 640x448, tests off,
    sprite (0,0)-(640,448) in `colour`."""
    regs = [
        (0x4C, 10 << 16),  # FRAME_1
        (0x4E, 1 << 32),  # ZBUF_1 ZMSK
        (0x40, 639 << 16 | 447 << 48),  # SCISSOR_1
        (0x18, 0),  # XYOFFSET_1
        (0x47, 1 << 16 | 1 << 17),  # TEST_1: ZTE, ZTST always
        (0x1A, 1),  # PRMODECONT
        (0x00, 6),  # PRIM sprite
        (0x01, colour | (0x3F800000 << 32)),  # RGBAQ (Q = 1.0)
        (0x05, 0),  # XYZ2 (0, 0)
        (0x05, (640 << 4) | (448 << 4) << 16),  # XYZ2 (640, 448) -> kick
    ]
    tag = struct.pack("<QQ", len(regs) | 0x8000 | (1 << 60), 0xE)
    return tag + b"".join(ad(r, v) for r, v in regs)


def program():
    """The loadable segment: code at BASE, the GIF packet at BASE + PACKET_OFFSET."""
    a = Allegrex(BASE)
    a.li(V1, 2)  # SetGsCrt(interlace=1, mode=2 NTSC, ffmd=0 frame)
    a.li(A0, 1)
    a.li(A1, 2)
    a.li(A2, 0)
    a.emit(0x0000000C)  # syscall
    a.nop()
    a.li(T0, GS_PRIV)
    a.li(T1, 0x200)  # CSR reset
    a.sw(T1, 0x1000, T0)
    a.li(T1, 1)  # SMODE2: interlaced, frame mode
    a.sw(T1, 0x0020, T0)
    a.sw(ZERO, 0x0024, T0)
    a.li(T1, 10 << 9)  # DISPFB1: FBP 0, FBW 10, PSM CT32
    a.sw(T1, 0x0070, T0)
    a.sw(ZERO, 0x0074, T0)
    disp = 636 | (50 << 12) | (3 << 23) | (2559 << 32) | (447 << 44)
    a.li(T1, disp & 0xFFFFFFFF)
    a.sw(T1, 0x0080, T0)
    a.li(T1, disp >> 32)
    a.sw(T1, 0x0084, T0)
    a.li(T1, 0x00004080)  # BGCOLOR: dark green fallback (R 0x80 G 0x40 B 0)
    a.sw(T1, 0x00E0, T0)
    a.sw(ZERO, 0x00E4, T0)
    a.li(T1, 0xFF25)  # PMODE: EN1, CRTMD, MMOD, ALP 0xFF
    a.sw(T1, 0x0000, T0)
    a.sw(ZERO, 0x0004, T0)
    a.li(T2, UNCACHED | DEBUG_BLOCK)
    a.li(T1, 0x5954534E)  # 'NSTY'
    a.sw(T1, 0, T2)
    a.sw(ZERO, 4, T2)  # frame counter
    a.li(T4, FIRST_COLOUR)
    a.li(T5, UNCACHED | BASE + PACKET_OFFSET)  # packet (uncached view)
    a.li(T6, DMAC_GIF)
    a.label("frame")
    a.label("vwait")  # wait VSINT (CSR bit 3)
    a.lw(T1, 0x1000, T0)
    a.andi(T1, T1, 8)
    a.beq(T1, ZERO, "vwait")
    a.li(T1, 8)  # ack VSINT (write bit 3)
    a.sw(T1, 0x1000, T0)
    a.lw(T1, 4, T2)  # counter++
    a.addiu(T1, T1, 1)
    a.sw(T1, 4, T2)
    a.addiu(T4, T4, 3)  # colour += 3 (red channel ramps, wraps into green)
    a.sw(T4, RGBAQ_INDEX * 16, T5)  # RGBAQ low word (R,G,B,A)
    a.li(T1, BASE + PACKET_OFFSET)  # D2_MADR (physical)
    a.sw(T1, 0x10, T6)
    a.li(T1, 11)  # D2_QWC: tag + 10 regs
    a.sw(T1, 0x20, T6)
    a.li(T1, 0x101)  # D2_CHCR: DIR to GIF, normal mode, STR
    a.sw(T1, 0x00, T6)
    a.label("dwait")  # wait DMA done (STR clears)
    a.lw(T1, 0x00, T6)
    a.andi(T1, T1, 0x100)
    a.bne(T1, ZERO, "dwait")
    a.j("frame")
    code = a.code()
    assert len(code) <= PACKET_OFFSET, "code overlaps the GIF packet"
    return bytes(code.ljust(PACKET_OFFSET, b"\0")) + gif_packet(FIRST_COLOUR)


def elf(segment, entry=BASE):
    """Plain ELF32 LE, EM_MIPS, ET_EXEC, one PT_LOAD (RWX) at BASE from SEG_OFFSET."""
    head = bytearray(SEG_OFFSET)
    head[:16] = b"\x7fELF\x01\x01\x01" + bytes(9)
    struct.pack_into("<HHIIIIIHHHHHH", head, 16, 2, 8, 1, entry, 0x34, 0, 0x20924001,
                     0x34, 0x20, 1, 0x28, 0, 0)
    struct.pack_into("<IIIIIIII", head, 0x34, 1, SEG_OFFSET, BASE, BASE,
                     len(segment), len(segment), 7, 0x10)
    return bytes(head) + segment
