"""Allegrex (PSP MIPS32, little-endian) emitter and PSP ELF writer for the Nostyra
PSP test program.

Part of build-nostyra-test-psp.py (self-written, GPLv3, Python 3 std-only).

ELF layout PPSSPP accepts without section headers: ET_EXEC at 0x08804000, one
PT_LOAD, module info found through the first segment's p_paddr (= its file offset).
Imports are 5-word stub entries {name*, ver, flags, size=5, nvars=0, nfuncs, nids*,
stubs*}; PPSSPP writes `jr ra; syscall` into each 8-byte stub slot at load.
"""
import struct

BASE = 0x08804000
SEG_OFFSET = 0x100  # file offset of the PT_LOAD segment
# Registers.
ZERO, V0, A0, A1, A2, A3 = 0, 2, 4, 5, 6, 7
T0, T1, T2, T3, T4, T5, T6, T7 = 8, 9, 10, 11, 12, 13, 14, 15
S0, S1, S2, S3, S4, S5, S6, S7, RA = 16, 17, 18, 19, 20, 21, 22, 23, 31


class Allegrex:
    """Little-endian MIPS emitter: labels with branch/jump fixups; explicit delay slots."""

    def __init__(self, base):
        self.base, self.words, self.labels, self.fixups = base, [], {}, []

    def emit(self, word):
        self.words.append(word & 0xFFFFFFFF)

    def here(self):
        return self.base + 4 * len(self.words)

    def label(self, name):
        self.labels[name] = self.here()

    def i(self, op, rs, rt, imm):
        self.emit((op << 26) | (rs << 21) | (rt << 16) | (imm & 0xFFFF))

    def r(self, funct, rs=0, rt=0, rd=0, sh=0):
        self.emit((rs << 21) | (rt << 16) | (rd << 11) | (sh << 6) | funct)

    def nop(self):
        self.emit(0)

    def li(self, rt, value):  # lui rt, hi ; ori rt, rt, lo
        self.i(0x0F, 0, rt, (value >> 16) & 0xFFFF)
        self.i(0x0D, rt, rt, value)

    def move(self, rd, rs):  # addu rd, rs, zero
        self.r(0x21, rs=rs, rd=rd)

    def addiu(self, rt, rs, imm):
        self.i(0x09, rs, rt, imm)

    def andi(self, rt, rs, imm):
        self.i(0x0C, rs, rt, imm)

    def slti(self, rt, rs, imm):
        self.i(0x0A, rs, rt, imm)

    def sltiu(self, rt, rs, imm):
        self.i(0x0B, rs, rt, imm)

    def or_(self, rd, rs, rt):
        self.r(0x25, rs=rs, rt=rt, rd=rd)

    def sll(self, rd, rt, sh):
        self.r(0x00, rt=rt, rd=rd, sh=sh)

    def lw(self, rt, off, base):
        self.i(0x23, base, rt, off)

    def lbu(self, rt, off, base):
        self.i(0x24, base, rt, off)

    def sw(self, rt, off, base):
        self.i(0x2B, base, rt, off)

    def beq(self, rs, rt, target):
        self.branch(0x04, rs, rt, target)

    def bne(self, rs, rt, target):
        self.branch(0x05, rs, rt, target)

    def branch(self, op, rs, rt, target):  # + delay-slot nop
        self.fixups.append((len(self.words), "b", target))
        self.i(op, rs, rt, 0)
        self.nop()

    def jump(self, op, target):  # j (2) / jal (3) + delay-slot nop
        self.fixups.append((len(self.words), "j", target))
        self.emit(op << 26)
        self.nop()

    def j(self, target):
        self.jump(0x02, target)

    def jal(self, target):
        self.jump(0x03, target)

    def code(self):
        for index, kind, target in self.fixups:
            dest = self.labels[target]
            if kind == "b":
                offset = (dest - (self.base + 4 * index + 4)) // 4
                self.words[index] |= offset & 0xFFFF
            else:
                self.words[index] |= (dest & 0x0FFFFFFF) >> 2
        return b"".join(struct.pack("<I", w) for w in self.words)


class Imports:
    """Import tables: `stub(module, nid)` returns the label of the call's stub slot."""

    def __init__(self):
        self.modules = {}  # module name -> [(label, nid)]

    def stub(self, module, name, nid):
        self.modules.setdefault(module, []).append((name, nid))
        return name

    def layout(self, base):
        """(bytes, {label: address}, (stub_table_start, end)) placed at `base`."""
        blob, labels = bytearray(), {}
        mods = list(self.modules.items())
        names_at, nids_at, slots_at = {}, {}, {}
        for module, _ in mods:  # module name strings
            names_at[module] = base + len(blob)
            blob += module.encode("ascii") + b"\0"
        blob += b"\0" * (-len(blob) % 4)
        for module, funcs in mods:  # NID arrays
            nids_at[module] = base + len(blob)
            blob += b"".join(struct.pack("<I", nid) for _, nid in funcs)
        for module, funcs in mods:  # 8-byte stub slots (filled by the loader)
            slots_at[module] = base + len(blob)
            for name, _ in funcs:
                labels[name] = base + len(blob)
                blob += struct.pack("<II", 0x03E00008, 0)  # jr ra; nop until patched
        table = base + len(blob)
        for module, funcs in mods:  # stub entries, 5 words each
            blob += struct.pack("<IHHBBHII", names_at[module], 0x0011, 0x4001, 5, 0,
                                len(funcs), nids_at[module], slots_at[module])
        return bytes(blob), labels, (table, base + len(blob))


def module_info(name, stubs):
    """PspModuleInfo: attr, version, name[28], gp, libent, libentend, libstub, libstubend."""
    return struct.pack("<HH28sIIIII", 0, 0x0101, name.encode("ascii"), 0, 0, 0, *stubs)


def elf(segment, entry, modinfo_offset):
    """ELF32 LE, EM_MIPS, ET_EXEC, one PT_LOAD at BASE from file offset SEG_OFFSET;
    `modinfo_offset` = module info offset inside `segment`."""
    head = bytearray(SEG_OFFSET)
    head[:16] = b"\x7fELF\x01\x01\x01" + bytes(9)
    struct.pack_into("<HHIIIIIHHHHHH", head, 16, 2, 8, 1, entry, 0x34, 0, 0x10A23001,
                     0x34, 0x20, 1, 0x28, 0, 0)
    struct.pack_into("<IIIIIIII", head, 0x34, 1, SEG_OFFSET, BASE,
                     SEG_OFFSET + modinfo_offset, len(segment), len(segment), 7, 0x10)
    return bytes(head) + segment
