"""ARM9 program of the Nostyra DS test ROMs (see build-nostyra-test-nds.py).

Engine A (top screen) backdrop = red 0x001F, green 0x03E0 while any KEYINPUT key
is held; frame counter (low 3 bits) in the blue channel. Engine B (bottom screen)
backdrop = blue 0x7C00 + frame bits in red while the pen is up; while the pen is
down = touch position from the ARM7: red = x / 8, green = y / 8. A white 3D
triangle (BG0 = 3D) in the top-left of the top screen, one vertex moving with the
frame counter: exercises the (threaded) 3D renderer. The frame counter word is also
stored at main RAM 0x023F0008 (RetroAchievements address 0x3F0008) every frame.
"""
from arm_emitter import (COND_AL, COND_EQ, COND_NE, OP_ADD, OP_AND, OP_CMP, OP_MOV, OP_ORR,
                         OP_TST, SHIFT_LSL, SHIFT_LSR, Arm, dpi, dpr, ldr, ldrh, str_, strh)

IO = 0x04000000
SHARED = 0x023F0000      # main RAM word: ARM7 -> ARM9 touch state; +4 boot counter
FRAME_COUNTER_OFFSET = 8  # SHARED + 8: frame counter word (RetroAchievements 0x3F0008)
ARM9_RAM = 0x02000000
VBLANK_LINE = 192


def wait_vcount(a, eq):
    """Spin until VCOUNT (r0 = IO) is (eq) / is not (not eq) the VBlank line."""
    name = f"vc{len(a.words)}"
    a.label(name)
    a.emit(ldrh(2, 0, 0x06))
    a.emit(dpi(OP_CMP, 0, 2, VBLANK_LINE, s=1))
    a.b(COND_NE if eq else COND_EQ, name)


def arm9_program(base=ARM9_RAM):
    """The ARM9 program assembled to run at `base`."""
    a = Arm(base)
    a.ldr_const(0, IO)
    a.ldr_const(1, IO + 0x304)
    a.ldr_const(3, 0x820F)
    a.emit(strh(3, 1))                            # POWCNT1: LCDs + 2D A/B, A on top
    a.ldr_const(3, 0x00010000)
    a.ldr_const(1, IO + 0x1000)
    a.emit(str_(3, 1))                            # DISPCNT B: display on, no layers
    a.ldr_const(3, 0x00010108)
    a.emit(str_(3, 0))                            # DISPCNT A: display on, BG0 = 3D
    a.emit(dpi(OP_MOV, 3, 0, 0))
    a.emit(str_(3, 0, 0x60))                      # DISP3DCNT
    a.emit(str_(3, 0, 0x350))                     # CLEAR_COLOR: transparent
    a.ldr_const(3, 0x7FFF)
    a.emit(str_(3, 0, 0x354))                     # CLEAR_DEPTH
    a.ldr_const(1, IO + 0x400)                    # r1 = geometry command ports
    a.ldr_const(3, 0xBFFF0000)
    a.emit(str_(3, 1, 0x180))                     # VIEWPORT 0,0,255,191
    a.ldr_const(4, 0x05000000)                    # r4 = backdrop A (top)
    a.ldr_const(5, 0x05000400)                    # r5 = backdrop B (bottom)
    a.ldr_const(10, IO + 0x130)                   # r10 = KEYINPUT
    a.ldr_const(11, SHARED)
    a.ldr_const(8, 0x3FF)
    a.emit(dpi(OP_MOV, 6, 0, 0))                  # r6 = frame counter
    a.label("loop")
    wait_vcount(a, True)
    a.emit(dpi(OP_ADD, 6, 6, 1))                  # ADD r6,r6,#1
    a.emit(str_(6, 11, FRAME_COUNTER_OFFSET))     # STR r6,[r11,#8] (frame counter in RAM)
    a.emit(ldrh(7, 10))
    a.emit(dpr(OP_CMP, 0, 7, 8, s=1))             # CMP r7,r8 (0x3FF = nothing held)
    a.emit(dpi(OP_MOV, 9, 0, 0x001F))             # top red
    a.emit(dpi(OP_MOV, 9, 0, 0x03E0, cond=COND_NE))  # key held -> green
    a.emit(dpi(OP_AND, 3, 6, 7))
    a.emit(dpr(OP_ORR, 9, 9, 3, SHIFT_LSL, 10))   # frame bits in blue
    a.emit(strh(9, 4))
    a.emit(ldr(3, 11))                            # touch word from the ARM7
    a.emit(dpi(OP_TST, 0, 3, 0x8000, s=1))
    a.b(COND_NE, "pen")
    a.emit(dpi(OP_AND, 9, 6, 7))
    a.emit(dpi(OP_ORR, 9, 9, 0x7C00))             # bottom blue + frame bits in red
    a.b(COND_AL, "bottom")
    a.label("pen")
    a.emit(dpi(OP_AND, 9, 3, 0x7C))               # x7 >> 2 = x / 8 (as red)
    a.emit(dpr(OP_MOV, 9, 0, 9, SHIFT_LSR, 2))
    a.emit(dpr(OP_MOV, 7, 0, 3, SHIFT_LSR, 9))    # y7 >> 2 = y / 8 (as green)
    a.emit(dpi(OP_AND, 7, 7, 0x1F))
    a.emit(dpr(OP_ORR, 9, 9, 7, SHIFT_LSL, 5))
    a.label("bottom")
    a.emit(strh(9, 5))
    triangle(a)
    wait_vcount(a, False)
    a.b(COND_AL, "loop")
    return a.resolve()


def triangle(a):
    """White 3D triangle in the top-left of the top screen; one vertex moves with
    the frame counter (r6) -> the 3D renderer (threaded or not) draws every frame."""
    a.emit(dpi(OP_MOV, 3, 0, 0))
    a.emit(str_(3, 1, 0x40))                      # MTX_MODE projection
    a.emit(str_(3, 1, 0x54))                      # MTX_IDENTITY
    a.emit(dpi(OP_MOV, 2, 0, 2))
    a.emit(str_(2, 1, 0x40))                      # MTX_MODE position + vector
    a.emit(str_(3, 1, 0x54))                      # MTX_IDENTITY
    a.ldr_const(2, 0x001F00C0)
    a.emit(str_(2, 1, 0xA4))                      # POLYGON_ATTR: both sides, alpha 31
    a.emit(str_(3, 1, 0x100))                     # BEGIN_VTXS triangles
    a.ldr_const(2, 0x7FFF)
    a.emit(str_(2, 1, 0x80))                      # COLOR white
    a.ldr_const(2, 0x1000F000)                    # (-1, 1)
    a.emit(str_(2, 1, 0x8C))
    a.emit(str_(3, 1, 0x8C))
    a.emit(dpi(OP_AND, 2, 6, 0x7F))
    a.emit(dpr(OP_MOV, 2, 0, 2, SHIFT_LSL, 5))
    a.ldr_const(12, 0x1000F200)                   # (-0.875 + frame/128, 1)
    a.emit(dpr(OP_ADD, 2, 2, 12))
    a.emit(str_(2, 1, 0x8C))
    a.emit(str_(3, 1, 0x8C))
    a.ldr_const(2, 0x0800F000)                    # (-1, 0.5)
    a.emit(str_(2, 1, 0x8C))
    a.emit(str_(3, 1, 0x8C))
    a.emit(str_(3, 1, 0x104))                     # END_VTXS
    a.emit(str_(3, 1, 0x140))                     # SWAP_BUFFERS
