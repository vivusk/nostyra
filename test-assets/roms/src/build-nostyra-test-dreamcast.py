#!/usr/bin/env python3
"""Builds the Nostyra Dreamcast test program: self-written, GPLv3, no toolchain.

Same approach as the other test ROM builders: Python 3 only, byte-for-byte
reproducible output, hand-assembled SH-4 instructions with mnemonics in the
comments. Outputs (next to this script's parent folder):
- nostyra-test-dc.elf: the program as an ELF (Flycast's HLE BIOS copies every PT_LOAD
  segment and jumps to 0x8C010000).
- nostyra-test-dc.gdi + "nostyra-test-dc (Track 1).bin" / "(Track 2).raw" /
  "(Track 3).bin": a TOSEC-style 3-track GD-ROM; track 3 at LBA 45000 holds IP.BIN
  (product NSTY-00001, area JUE, boot file 1ST_READ.BIN) and a tiny ISO9660 volume
  with the program as 1ST_READ.BIN. The HLE BIOS loads it; no licence bootstrap.
- nostyra-test-dc.cue: the same three tracks as a Redump-style sheet (density REMs).
- nostyra-test-dc-disc2.gdi (+ tracks): disc 2 (NSTY-00002) whose bottom half is
  magenta instead of blue; nostyra-test-dc.m3u lists both discs.
No Sega code or data: IP.BIN fields follow the public layout, the bootstrap area
stays zero. This program never runs on real hardware.

What the program does (each feature backs a runner test on Flycast):
- Video: PVR framebuffer 640x480 RGB565 at VRAM offset 0 (no TA / 3D). After each
  vertical blank the CPU fills the top half red and the bottom half blue whose green
  channel is the frame counter (mod 32), so consecutive frames differ and an
  upside-down image shows.
- RAM: the frame counter word at 0x8C0F0000 (RetroAchievements address 0x0F0000).
- Input: controller on port A read every frame by Maple DMA (GET_CONDITION):
  A held -> top half green; right trigger past half -> top half yellow; stick X
  right -> bottom half white.

Usage: python build-nostyra-test-dreamcast.py [output folder]
"""
import hashlib
import pathlib
import sys

from dreamcast_test_disc import gdi_disc, redump_cue
from dreamcast_test_sh4 import ENTRY, Sh4, elf

WIDTH, HEIGHT = 640, 480
HALF_WORDS = WIDTH * HEIGHT // 2 // 2  # 32-bit stores per half screen (2 px each)
# RGB565 colours, two pixels per 32-bit word.
RED, GREEN, YELLOW, WHITE = 0xF800F800, 0x07E007E0, 0xFFE0FFE0, 0xFFFFFFFF
BLUE, MAGENTA = 0x001F, 0xF81F  # bottom half base colour: disc 1 / disc 2
STICK_RIGHT = 0xC0  # analog X 0..255, centre 0x80
TRIGGER_HALF = 0x80  # trigger 0..255
# Holly / PVR registers (P2, uncached).
PVR = 0xA05F8000
FB_R_CTRL, FB_R_SOF1, FB_R_SIZE, SPG_STATUS = PVR + 0x44, PVR + 0x50, PVR + 0x5C, PVR + 0x10C
VO_CONTROL = PVR + 0xE8
VO_CONTROL_VAL = 0x00000100  # reset value 0x108 has blank_video (bit 3) set
FB_R_CTRL_VAL = 0x00800005  # enable, RGB565, VGA pixel clock (480 progressive lines)
SPG_LOAD = PVR + 0xD8
SPG_LOAD_VAL = 0x020C0359  # 525 lines x 858 clocks: 60 frames per second at the VGA clock
VO_STARTX, VO_STARTY = PVR + 0xEC, PVR + 0xF0
VO_STARTX_VAL, VO_STARTY_VAL = 0xA8, 0x00280028  # VGA picture start: no shift
FB_R_SIZE_VAL = (1 << 20) | ((HEIGHT - 1) << 10) | (WIDTH * 2 // 4 - 1)
SCANLINE_MASK = 0x3FF  # SPG_STATUS bits 9-0; Flycast presents at scanline 0
VRAM32 = 0xA5000000
SB_MDSTAR, SB_MDTSEL, SB_MDEN, SB_MDST = 0xA05F6C04, 0xA05F6C10, 0xA05F6C14, 0xA05F6C18
MAPLE_TABLE = 0x0C100000  # physical; written and read through P2 (0xA0000000 | ...)
MAPLE_RECV = MAPLE_TABLE + 0x40
P2 = 0xA0000000
FRAME_COUNTER = 0xAC0F0000  # P2 view of main RAM 0x0C0F0000 (RetroAchievements 0x0F0000)


def program(bottom=BLUE):
    a = Sh4(ENTRY)
    # Framebuffer: 640x480 RGB565 at VRAM 0.
    a.store(FB_R_SIZE, FB_R_SIZE_VAL)
    a.store(FB_R_SOF1, 0)
    a.store(SPG_LOAD, SPG_LOAD_VAL)
    a.store(FB_R_CTRL, FB_R_CTRL_VAL)
    a.store(VO_CONTROL, VO_CONTROL_VAL)
    a.store(VO_STARTX, VO_STARTX_VAL)
    a.store(VO_STARTY, VO_STARTY_VAL)
    # Maple DMA table: one GET_CONDITION (cmd 9) to port A's controller (0x20), last.
    for i, word in enumerate([0x80000001, MAPLE_RECV, 0x01002009, 0x01000000]):
        a.store(P2 | MAPLE_TABLE + 4 * i, word)
    a.store(SB_MDSTAR, MAPLE_TABLE)
    a.store(SB_MDTSEL, 0)  # software-triggered
    a.store(SB_MDEN, 1)
    a.mov_imm(8, 0)  # r8 = frame counter

    a.label("frame")
    # Wait for the next frame: scanline back to 0 (Flycast presents there), then past it.
    # (The vsync bit is unusable: the reset SPG_VBLANK end line is beyond the last line.)
    a.li(1, SPG_STATUS)
    a.li(5, SCANLINE_MASK)
    a.label("in_vblank")
    a.ld_l(1, 0)
    a.tst(5, 0)
    a.branch("bf", "in_vblank")  # scanline != 0
    a.label("in_display")
    a.ld_l(1, 0)
    a.tst(5, 0)
    a.branch("bt", "in_display")  # still on scanline 0
    # Pad: preset "nothing pressed, stick centred", then one Maple DMA.
    a.store(P2 | MAPLE_RECV + 8, 0xFFFFFFFF)  # buttons are active-low
    a.store(P2 | MAPLE_RECV + 12, 0x80808080)
    a.store(SB_MDST, 1)
    a.li(3, 100000)  # timeout
    a.label("maple_wait")
    a.ld_l(1, 0)
    a.tst(0, 0)
    a.branch("bt", "maple_done")
    a.dt(3)
    a.branch("bf", "maple_wait")
    a.label("maple_done")
    a.li(1, P2 | MAPLE_RECV + 8)
    a.ld_l(1, 9)  # r9 = buttons | rtrig << 16 | ltrig << 24
    a.add_imm(1, 4)
    a.ld_l(1, 10)  # r10 = stick x | y << 8 | ...
    # Top half: red, green while A is held (bit 2 clear), yellow while the right
    # trigger is past half.
    a.li(2, RED)
    a.mov(9, 0)
    a.tst_r0(0x04)
    a.branch("bf", "a_released")  # bit set = released
    a.li(2, GREEN)
    a.label("a_released")
    a.mov(9, 0)
    a.shlr16(0)
    a.and_r0(0xFF)  # r0 = right trigger
    a.li(4, TRIGGER_HALF)
    a.cmp_hs(4, 0)
    a.branch("bf", "top_fill")
    a.li(2, YELLOW)
    a.label("top_fill")
    a.li(1, VRAM32)
    a.li(3, HALF_WORDS)
    a.label("top_loop")
    a.st_l(2, 1)
    a.add_imm(1, 4)
    a.dt(3)
    a.branch("bf", "top_loop")
    # Bottom half: base colour + (counter mod 32) in green, white while the stick is
    # right.
    a.mov(8, 0)
    a.and_r0(31)
    a.shll2(0)
    a.shll2(0)
    a.shll(0)  # << 5: low bits of the 6-bit green field
    a.li(2, bottom)
    a.or_(0, 2)
    a.mov(2, 0)
    a.shll16(0)
    a.or_(0, 2)  # same colour in both pixels
    a.mov(10, 0)
    a.and_r0(0xFF)
    a.li(4, STICK_RIGHT)
    a.cmp_hs(4, 0)
    a.branch("bf", "bottom_fill")
    a.li(2, WHITE)
    a.label("bottom_fill")
    a.li(3, HALF_WORDS)
    a.label("bottom_loop")
    a.st_l(2, 1)
    a.add_imm(1, 4)
    a.dt(3)
    a.branch("bf", "bottom_loop")
    a.add_imm(8, 1)
    a.li(1, FRAME_COUNTER)
    a.st_l(8, 1)  # frame counter word in main RAM
    a.branch("bra", "frame")
    a.nop()  # delay slot
    return a.assemble()


def main():
    out_dir = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parent.parent
    code = program()
    data = elf(code)
    path = out_dir / "nostyra-test-dc.elf"
    path.write_bytes(data)

    disc1 = gdi_disc(out_dir, "nostyra-test-dc", "NSTY-00001", "NOSTYRA TEST DISC", code)
    (out_dir / "nostyra-test-dc.cue").write_text(redump_cue(disc1), newline="\n")
    gdi_disc(out_dir, "nostyra-test-dc-disc2", "NSTY-00002", "NOSTYRA TEST DISC 2", program(MAGENTA))
    playlist = "nostyra-test-dc.gdi\nnostyra-test-dc-disc2.gdi\n"
    (out_dir / "nostyra-test-dc.m3u").write_text(playlist, newline="\n")
    for name in sorted(p.name for p in out_dir.glob("nostyra-test-dc*")):
        print(f"{name} sha1 {hashlib.sha1((out_dir / name).read_bytes()).hexdigest()}")


if __name__ == "__main__":
    main()
