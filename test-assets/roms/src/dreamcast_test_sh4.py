"""SH-4 emitter (16-bit opcodes, labels, PC-relative literal pool) and the ELF
writer for the Nostyra Dreamcast test program.

Part of build-nostyra-test-dreamcast.py (self-written, GPLv3, Python 3 std-only).
"""
import struct

ENTRY = 0x8C010000  # reios jumps here whatever e_entry says


class Sh4:
    """Tiny SH-4 emitter: 16-bit opcodes, labels, a PC-relative literal pool."""

    def __init__(self, base):
        self.base, self.ops, self.labels, self.fixups, self.literals = base, [], {}, [], []

    def op(self, word):
        self.ops.append(word & 0xFFFF)

    def label(self, name):
        self.labels[name] = len(self.ops)

    def li(self, rn, value):
        """mov.l @(disp,PC),Rn -- 32-bit constant from the pool."""
        self.fixups.append(("pool", len(self.ops), value & 0xFFFFFFFF))
        self.op(0xD000 | rn << 8)

    def branch(self, kind, name):
        """bt/bf (no delay slot) or bra (delay slot: caller emits it)."""
        self.fixups.append((kind, len(self.ops), name))
        self.op({"bt": 0x8900, "bf": 0x8B00, "bra": 0xA000}[kind])

    # Plain instructions (n = destination, m = source).
    def mov_imm(self, n, imm): self.op(0xE000 | n << 8 | (imm & 0xFF))  # mov #imm,Rn
    def mov(self, m, n): self.op(0x6003 | n << 8 | m << 4)  # mov Rm,Rn
    def st_l(self, m, n): self.op(0x2002 | n << 8 | m << 4)  # mov.l Rm,@Rn
    def ld_l(self, m, n): self.op(0x6002 | n << 8 | m << 4)  # mov.l @Rm,Rn
    def add_imm(self, n, imm): self.op(0x7000 | n << 8 | (imm & 0xFF))  # add #imm,Rn
    def dt(self, n): self.op(0x4010 | n << 8)  # dt Rn
    def tst(self, m, n): self.op(0x2008 | n << 8 | m << 4)  # tst Rm,Rn
    def tst_r0(self, imm): self.op(0xC800 | imm)  # tst #imm,R0
    def and_r0(self, imm): self.op(0xC900 | imm)  # and #imm,R0
    def or_(self, m, n): self.op(0x200B | n << 8 | m << 4)  # or Rm,Rn
    def cmp_hs(self, m, n): self.op(0x3002 | n << 8 | m << 4)  # cmp/hs Rm,Rn (Rn >= Rm)
    def shll2(self, n): self.op(0x4008 | n << 8)  # shll2 Rn
    def shll(self, n): self.op(0x4000 | n << 8)  # shll Rn
    def shll16(self, n): self.op(0x4028 | n << 8)  # shll16 Rn
    def shlr16(self, n): self.op(0x4029 | n << 8)  # shlr16 Rn
    def nop(self): self.op(0x0009)

    def store(self, addr, value):
        """*(u32 *)addr = value, through r1/r0."""
        self.li(1, addr)
        self.li(0, value)
        self.st_l(0, 1)

    def assemble(self):
        code_len = len(self.ops) * 2
        pool_at = (code_len + 3) & ~3
        pool = []
        for kind, at, arg in self.fixups:
            pc = self.base + at * 2
            if kind == "pool":
                if arg not in pool:
                    pool.append(arg)
                target = self.base + pool_at + pool.index(arg) * 4
                disp = (target - ((pc & ~3) + 4)) // 4
                assert 0 <= disp <= 0xFF, "literal out of range"
                self.ops[at] |= disp
                continue
            disp = (self.base + self.labels[arg] * 2 - (pc + 4)) // 2
            if kind == "bra":
                assert -2048 <= disp < 2048
                self.ops[at] |= disp & 0xFFF
            else:
                assert -128 <= disp < 128, f"branch to {arg} out of range"
                self.ops[at] |= disp & 0xFF
        body = b"".join(struct.pack("<H", o) for o in self.ops)
        body += b"\0" * (pool_at - code_len)
        return body + b"".join(struct.pack("<I", v) for v in pool)


def elf(code):
    """ELF32 little-endian SH executable: one PT_LOAD at ENTRY, sections null/.text/
    .shstrtab (Flycast's libelf refuses a file without a section-name table)."""
    offset = 0x100
    names = b"\0.text\0.shstrtab\0"
    names_at = offset + len(code)
    sh_at = (names_at + len(names) + 3) & ~3
    header = b"\x7fELF" + bytes([1, 1, 1, 0]) + bytes(8)
    header += struct.pack("<HHIIIIIHHHHHH", 2, 42, 1, ENTRY, 52, sh_at, 0, 52, 32, 1, 40, 3, 2)
    phdr = struct.pack("<IIIIIIII", 1, offset, ENTRY, ENTRY, len(code), len(code), 7, 4)
    image = header + phdr
    image += bytes(offset - len(image)) + code + names
    image += bytes(sh_at - len(image))
    image += bytes(40)  # null section
    image += struct.pack("<IIIIIIIIII", 1, 1, 6, ENTRY, offset, len(code), 0, 0, 4, 0)  # .text
    image += struct.pack("<IIIIIIIIII", 7, 3, 0, 0, names_at, len(names), 0, 0, 1, 0)  # .shstrtab
    return image
