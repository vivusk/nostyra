"""Minimal 32-bit PowerPC (GameCube Gekko / Wii Broadway) assembler for the GameCube
and Wii test DOLs.

Hand-assembled big-endian words (mnemonics as method names), labels with branch
fixups resolved by `assemble()`. Only the instructions the test program needs; no
external toolchain. Ported from the M14 phase-00 spike (branch spike/m14-dolphin).
"""
import struct


class Ppc:
    """Word list + labels; branch words are `(encoder, label)` until `assemble()`."""

    def __init__(self, base):
        self.base = base
        self.words = []  # int, or (fn(displacement) -> word, label name)
        self.labels = {}

    # ---- plumbing ----
    def here(self):
        """Address of the next word."""
        return self.base + 4 * len(self.words)

    def label(self, name):
        assert name not in self.labels, name
        self.labels[name] = self.here()

    def word(self, w):
        self.words.append(w & 0xFFFFFFFF)

    def align(self, boundary):
        """Pads with zero words up to `boundary` bytes."""
        while self.here() % boundary:
            self.word(0)

    def assemble(self):
        """Big-endian bytes with every branch displacement filled in."""
        out = bytearray()
        for i, w in enumerate(self.words):
            if isinstance(w, tuple):
                fn, name = w
                w = fn(self.labels[name] - (self.base + 4 * i))
            out += struct.pack(">I", w & 0xFFFFFFFF)
        return bytes(out)

    # ---- D-form: opcode | rD/rS | rA | 16-bit immediate ----
    def _d(self, op, rd, ra, imm):
        self.word((op << 26) | (rd << 21) | (ra << 16) | (imm & 0xFFFF))

    def addi(self, rd, ra, imm):
        self._d(14, rd, ra, imm)

    def li(self, rd, imm):  # addi rd, 0, imm (sign-extended)
        self.addi(rd, 0, imm)

    def addis(self, rd, ra, imm):
        self._d(15, rd, ra, imm)

    def lis(self, rd, imm):  # addis rd, 0, imm
        self.addis(rd, 0, imm)

    def addic_(self, rd, ra, imm):  # addic. (sets CR0)
        self._d(13, rd, ra, imm)

    def ori(self, ra, rs, imm):
        self._d(24, rs, ra, imm)

    def andi_(self, ra, rs, imm):  # andi. (sets CR0)
        self._d(28, rs, ra, imm)

    def andis_(self, ra, rs, imm):  # andis. (sets CR0)
        self._d(29, rs, ra, imm)

    def lwz(self, rd, off, ra):
        self._d(32, rd, ra, off)

    def stw(self, rs, off, ra):
        self._d(36, rs, ra, off)

    def sth(self, rs, off, ra):
        self._d(44, rs, ra, off)

    def li32(self, rd, value):
        """lis + ori: any 32-bit constant (ori skipped when the low half is zero)."""
        value &= 0xFFFFFFFF
        self.lis(rd, value >> 16)
        if value & 0xFFFF:
            self.ori(rd, rd, value & 0xFFFF)

    # ---- rotate / X-form ----
    def rlwinm(self, ra, rs, sh, mb, me, rc=0):
        self.word((21 << 26) | (rs << 21) | (ra << 16) | (sh << 11) | (mb << 6) | (me << 1) | rc)

    def _x(self, rs, ra, rb, xo, rc=0):
        self.word((31 << 26) | (rs << 21) | (ra << 16) | (rb << 11) | (xo << 1) | rc)

    def or_(self, ra, rs, rb):
        self._x(rs, ra, rb, 444)

    def mr(self, ra, rs):  # or ra, rs, rs
        self.or_(ra, rs, rs)

    def _spr(self, reg, spr, xo):
        field = ((spr & 0x1F) << 5) | ((spr >> 5) & 0x1F)  # SPR halves swapped
        self.word((31 << 26) | (reg << 21) | (field << 11) | (xo << 1))

    def mtspr(self, spr, rs):
        self._spr(rs, spr, 467)

    def mfspr(self, rd, spr):
        self._spr(rd, spr, 339)

    # ---- branches to labels ----
    def b(self, name):
        self.words.append((lambda d: (18 << 26) | (d & 0x3FFFFFC), name))

    def _bc(self, bo, bi, name):
        self.words.append((lambda d: (16 << 26) | (bo << 21) | (bi << 16) | (d & 0xFFFC), name))

    def beq(self, name, crf=0):  # branch if CR[crf].EQ
        self._bc(12, 4 * crf + 2, name)

    def bne(self, name, crf=0):  # branch if not CR[crf].EQ
        self._bc(4, 4 * crf + 2, name)
