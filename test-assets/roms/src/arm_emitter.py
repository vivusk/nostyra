"""Minimal 32-bit ARM assembler shared by the GBA and DS test ROM builders.

Hand-assembled instructions (mnemonics in the callers' comments), labels with
branch fixups and an `ldr rd, =value` literal pool. No external toolchain.
"""
import struct

COND_AL, COND_EQ, COND_NE = 0xE, 0x0, 0x1
# Data-processing opcodes and shift types (ARM ARM A5.1).
OP_AND, OP_SUB, OP_ADD, OP_TST, OP_CMP, OP_ORR, OP_MOV = 0x0, 0x2, 0x4, 0x8, 0xA, 0xC, 0xD
SHIFT_LSL, SHIFT_LSR = 0, 1


class Arm:
    """Minimal ARM emitter: labels, branch fixups and an `ldr rd, =value` pool."""

    def __init__(self, base):
        self.base = base
        self.words = []
        self.labels = {}
        self.fixups = []  # (index, kind, target): kind "b" (label) or "lit" (value)

    @property
    def pc(self):
        return self.base + 4 * len(self.words)

    def label(self, name):
        self.labels[name] = self.pc

    def emit(self, word):
        self.words.append(word)

    def b(self, cond, name):
        self.fixups.append((len(self.words), "b", name))
        self.emit(cond << 28 | 0x0A000000)

    def ldr_const(self, rd, value):
        self.fixups.append((len(self.words), "lit", value))
        self.emit(0xE59F0000 | rd << 12)          # LDR rd,[pc,#pool]

    def resolve(self):
        pool_at = self.pc
        pool = []
        for index, kind, target in self.fixups:
            pc = self.base + 4 * index + 8
            if kind == "b":
                self.words[index] |= ((self.labels[target] - pc) >> 2) & 0xFFFFFF
                continue
            if target not in pool:
                pool.append(target)
            offset = pool_at + 4 * pool.index(target) - pc
            assert 0 <= offset < 0x1000
            self.words[index] |= offset
        return b"".join(struct.pack("<I", w) for w in self.words + pool)


def mov(rd, imm8):
    return 0xE3A00000 | rd << 12 | imm8          # MOV rd,#imm8


def cmp_imm(rn, imm8):
    return 0xE3500000 | rn << 16 | imm8          # CMP rn,#imm8


def add_imm(rd, rn, imm8):
    return 0xE2800000 | rn << 16 | rd << 12 | imm8  # ADD rd,rn,#imm8


def ldrb(rd, rn, off=0):
    return 0xE5D00000 | rn << 16 | rd << 12 | off   # LDRB rd,[rn,#off]


def strb(rd, rn, off=0):
    return 0xE5C00000 | rn << 16 | rd << 12 | off   # STRB rd,[rn,#off]


def halfword(base, rd, rn, off):
    return base | rn << 16 | rd << 12 | (off >> 4) << 8 | (off & 0xF)


def ldrh(rd, rn, off=0):
    return halfword(0xE1D000B0, rd, rn, off)     # LDRH rd,[rn,#off]


def strh(rd, rn, off=0):
    return halfword(0xE1C000B0, rd, rn, off)     # STRH rd,[rn,#off]


def ldr(rd, rn, off=0):
    return 0xE5900000 | rn << 16 | rd << 12 | off   # LDR rd,[rn,#off]


def str_(rd, rn, off=0):
    return 0xE5800000 | rn << 16 | rd << 12 | off   # STR rd,[rn,#off]


def imm12(value):
    """Rotated 8-bit immediate field for `value` (ValueError if not encodable)."""
    for rot in range(16):
        r = ((value << (2 * rot)) | (value >> (32 - 2 * rot))) & 0xFFFFFFFF if rot else value
        if r < 256:
            return rot << 8 | r
    raise ValueError(hex(value))


def dpi(op, rd, rn, value, s=0, cond=COND_AL):
    """Data processing, immediate operand: `op{cond}{s} rd, rn, #value`."""
    return cond << 28 | 1 << 25 | op << 21 | s << 20 | rn << 16 | rd << 12 | imm12(value)


def dpr(op, rd, rn, rm, shift=SHIFT_LSL, amount=0, s=0, cond=COND_AL):
    """Data processing, register operand: `op{cond}{s} rd, rn, rm, shift #amount`."""
    return (cond << 28 | op << 21 | s << 20 | rn << 16 | rd << 12 | amount << 7
            | shift << 5 | rm)
