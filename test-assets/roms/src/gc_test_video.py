"""Video side of the GameCube / Wii test program: VI NTSC 480i registers and the
YUY2 XFB drawing helpers (colour bars, filled blocks).

VI: NTSC 480i timing (libogc table values), interlaced 640x480 YUY2 XFB at phys
0x00300000; DI0 / DI1 raise one interrupt flag per field. Registers are poked
through r31 = 0xCC000000; scratch registers are named per helper.
"""

XFB_PHYS = 0x00300000
XFB = 0xC0000000 | XFB_PHYS  # uncached view
STRIDE = 1280  # 640 px * 2 B

# YUY2 words (Y0 U Y1 V)
WHITE, YELLOW, CYAN, GREEN = 0xEB80EB80, 0xD292D210, 0xA910A9A5, 0x91369122
MAGENTA, RED, BLUE, BLACK = 0x6ACA6ADE, 0x515A51F0, 0x29F0296E, 0x10801080
BARS = [WHITE, YELLOW, CYAN, GREEN, MAGENTA, RED, BLUE, BLACK]


def fill_rect(p, tag, x_words, y, w_words, h, colour_reg):
    """Fills w_words x h YUY2 words at (x_words * 2 px, y); uses r7..r9, r11."""
    p.li32(7, XFB + y * STRIDE + x_words * 4)
    p.li(8, h)
    p.label(f"{tag}_row")
    p.mr(11, 7)
    p.li(9, w_words)
    p.label(f"{tag}_col")
    p.stw(colour_reg, 0, 11)
    p.addi(11, 11, 4)
    p.addic_(9, 9, -1)
    p.bne(f"{tag}_col")
    p.addi(7, 7, STRIDE)
    p.addic_(8, 8, -1)
    p.bne(f"{tag}_row")


def setup_vi(p):
    """VI NTSC 480i, interlaced XFB, DI0 / DI1 = one interrupt flag per field."""
    regs = [
        (p.sth, 0x2000, (240 << 4) | 6),  # VTR: ACV 240, EQU 6
        (p.stw, 0x2004, (71 << 24) | (105 << 16) | 429),  # HTR0: HCS, HCE, HLW
        (p.stw, 0x2008, 64 | (162 << 7) | (373 << 17)),  # HTR1: HSY, HBE640, HBS640
        (p.stw, 0x200C, (3 << 16) | 24),  # VTO: PSB, PRB
        (p.stw, 0x2010, (2 << 16) | 25),  # VTE
        (p.stw, 0x2014, 12 | (520 << 5) | (12 << 16) | (520 << 21)),  # BBOI
        (p.stw, 0x2018, 13 | (519 << 5) | (13 << 16) | (519 << 21)),  # BBEI
        (p.stw, 0x201C, (1 << 28) | (XFB_PHYS >> 5)),  # TFBL (POFF: addr >> 5)
        (p.stw, 0x2024, (1 << 28) | ((XFB_PHYS + STRIDE) >> 5)),  # BFBL = next line
        (p.stw, 0x2030, (1 << 28) | (1 << 16) | 1),  # DI0: enable, VCT 1, HCT 1
        (p.stw, 0x2034, (1 << 28) | (264 << 16) | 1),  # DI1: VCT 264 (field 2)
        (p.sth, 0x2048, (40 << 8) | 80),  # PICCONF: WPL 40, STD 80 (interlaced)
        (p.sth, 0x204A, 256),  # HSR: step 256, no scaling
        (p.sth, 0x206C, 0),  # VICLK 27 MHz
        (p.sth, 0x2002, 1),  # DCR: enable, interlaced, NTSC
    ]
    for store, reg, value in regs:
        p.li32(3, value)
        store(3, reg, 31)


def draw_bars(p):
    """8 vertical colour bars (80 px each) over all 480 lines."""
    p.li32(12, XFB)
    p.li(13, 480)
    p.label("bars_row")
    p.mr(11, 12)
    for i, colour in enumerate(BARS):
        p.li32(10, colour)
        p.li(9, 40)
        p.label(f"bar{i}")
        p.stw(10, 0, 11)
        p.addi(11, 11, 4)
        p.addic_(9, 9, -1)
        p.bne(f"bar{i}")
    p.addi(12, 12, STRIDE)
    p.addic_(13, 13, -1)
    p.bne("bars_row")
