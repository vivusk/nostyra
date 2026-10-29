"""The GameCube / Wii test program (PowerPC, registers poked directly, no SDK).

- VI (gc_test_video.py): NTSC 480i, interlaced 640x480 YUY2 XFB at phys
  0x00300000: 8 colour bars, a frame block (64x64 at x 32, y 32) white on odd and
  blue on even VI fields, a pad block (64x64 at x 128, y 32) green while GC A is held,
  else red. The XFB changes every field: Dolphin presents only changed XFBs.
- SI: channel 0 GameCube pad poll; the raw IN_HI / IN_LO words go to MEM1.
- PI: the reset button (Dolphin's retro_reset taps it) is latched into MEM1.
- AI: DMA loop of a 256-frame square wave (+-0x2000, 48 kHz) from the text section
  (no DSP microcode needed).
- MEM1 debug block at phys 0x00200000 (= RETRO_MEMORY_SYSTEM_RAM offset): the
  word offsets below (DEBUG_PHYS, MAGIC ... RESET_SEEN) are mirrored in the runner
  suite `apps/runner/src/gamecube_wii_real_core/mod.rs`; change both together.
"""
import struct

from gc_test_video import BLUE, GREEN, RED, WHITE, draw_bars, fill_rect, setup_vi
from ppc_emitter import Ppc

TEXT = 0x80003100  # load + entry address (32-byte aligned)
HID4 = 1011  # SPR number; `mtspr HID4` in a text section = Wii executable

# MEM1 debug block: physical address and word offsets.
DEBUG_PHYS = 0x00200000
DEBUG = 0x80000000 | DEBUG_PHYS
MAGIC = 0x4E535459  # +0 'NSTY'
FIELDS = 4  # +4 VI field counter
SI_IN_HI = 8  # +8 SI channel 0 IN_HI (buttons, stick X, stick Y)
SI_IN_LO = 12  # +12 SI channel 0 IN_LO (C-stick X/Y, analog L, analog R)
SPINS = 16  # +16 wait-loop spins
PHASE = 20  # +20 setup phase (1 VI, 2 SI, 3 AI, 4 bars, 5 main loop)
LAST_DI0 = 24  # +24 last VI DI0 value read
HID4_AT_BOOT = 28  # +28 HID4 as the emulator set it (Wii mode: non-zero)
PI_CAUSE = 32  # +32 last PI interrupt cause (bit 16 = reset button up)
RESET_SEEN = 36  # +36 1 once the reset button was seen pressed (retro_reset)

def phase(p, value):
    p.li(3, value)
    p.stw(3, PHASE, 30)


def setup_si(p):
    """SI channel 0: GC pad command, poll every field, copy to the IN buffers."""
    p.li32(3, 0x00400300)
    p.stw(3, 0x6400, 31)
    p.li32(3, (0xF6 << 16) | (2 << 8) | 0x80 | 0x08)  # SIPOLL: X 246, Y 2, EN0, VBCPY0
    p.stw(3, 0x6430, 31)
    p.lis(3, 0x8000)  # SISR.WR: copy out buffers
    p.stw(3, 0x6438, 31)


def main_loop(p):
    """Waits for a DI0 / DI1 flag, then counts the field, mirrors SI, redraws."""
    p.label("loop")
    phase(p, 5)
    p.label("wait")
    p.addi(27, 27, 1)
    p.stw(27, SPINS, 30)
    p.lwz(3, 0x2030, 31)
    p.stw(3, LAST_DI0, 30)
    p.andis_(4, 3, 0x8000)
    p.beq("try_di1")
    p.rlwinm(3, 3, 0, 1, 31)  # clear the interrupt flag
    p.stw(3, 0x2030, 31)
    p.b("field")
    p.label("try_di1")
    p.lwz(3, 0x2034, 31)
    p.andis_(4, 3, 0x8000)
    p.beq("wait")
    p.rlwinm(3, 3, 0, 1, 31)
    p.stw(3, 0x2034, 31)
    p.label("field")
    p.addi(29, 29, 1)
    p.stw(29, FIELDS, 30)
    p.lwz(5, 0x6404, 31)  # SI IN_HI
    p.lwz(6, 0x6408, 31)  # SI IN_LO
    p.stw(5, SI_IN_HI, 30)
    p.stw(6, SI_IN_LO, 30)
    p.lwz(3, 0x3000, 31)  # PI interrupt cause
    p.stw(3, PI_CAUSE, 30)
    p.andis_(4, 3, 0x0001)  # reset button state: 1 = up
    p.bne("reset_up")
    p.li(4, 1)
    p.stw(4, RESET_SEEN, 30)
    p.label("reset_up")
    p.li32(10, BLUE)  # frame block: blue on even fields, white on odd ones
    p.andi_(4, 29, 0x01)
    p.beq("fb_draw")
    p.li32(10, WHITE)
    p.label("fb_draw")
    fill_rect(p, "fb", 16, 32, 32, 64, 10)
    p.li32(10, RED)  # pad block: green while A (IN_HI bit 24) is held
    p.andis_(4, 5, 0x0100)
    p.beq("pad_draw")
    p.li32(10, GREEN)
    p.label("pad_draw")
    fill_rect(p, "pad", 64, 32, 32, 64, 10)
    p.b("loop")


def program(wii):
    """Text section bytes; `wii` adds one never-executed `mtspr HID4, r0`."""
    p = Ppc(TEXT)
    p.lis(31, 0xCC00)  # hardware registers
    p.li32(30, DEBUG)
    p.li(29, 0)  # field counter
    p.li(27, 0)  # spins
    p.li32(3, MAGIC)
    p.stw(3, 0, 30)
    p.mfspr(3, HID4)
    p.stw(3, HID4_AT_BOOT, 30)
    setup_vi(p)
    phase(p, 1)
    setup_si(p)
    phase(p, 2)
    p.label("ai_setup")  # AI DMA start address patched below (table after the code)
    p.lis(3, 0)  # DSP_AI_DMA_START_H
    p.sth(3, 0x5030, 31)
    p.li(3, 0)  # DSP_AI_DMA_START_L
    p.sth(3, 0x5032, 31)
    p.li(3, 0x8000 | 32)  # DMA enable, 32 blocks of 32 B
    p.sth(3, 0x5036, 31)
    phase(p, 3)
    draw_bars(p)
    phase(p, 4)
    main_loop(p)
    if wii:
        p.mtspr(HID4, 0)  # never executed: marks the DOL as a Wii executable
    p.align(32)
    table = p.here()
    for i in range(256):  # stereo 16-bit frames, 32-frame half periods
        v = (0x2000 if (i // 32) % 2 == 0 else -0x2000) & 0xFFFF
        p.word((v << 16) | v)
    code = bytearray(p.assemble())
    phys = table & 0x01FFFFFF
    assert (phys & 0xFFFF) < 0x8000, "li sign-extends"
    ai = p.labels["ai_setup"] - TEXT
    struct.pack_into(">H", code, ai + 2, phys >> 16)  # lis r3, hi
    struct.pack_into(">H", code, ai + 10, phys & 0xFFFF)  # li r3, lo
    return bytes(code)
